//! One-shot synchronous executor adapting an async writer body into
//! `std::io::Read`, copying directly into the caller's buffer.
//!
//! Not `Send`/`Sync` by construction (`Rc` + `Cell`); the driver is the
//! only thing that ever polls the future, and the `Writer` can only
//! observe the smuggled buffer pointer *during* a poll, which is the
//! only window in which it is installed.

use std::{
	cell::Cell,
	future::Future,
	io,
	pin::Pin,
	ptr::{self, NonNull},
	rc::Rc,
	task::{Context, Poll, Waker},
};

struct Shared {
	/// Caller's `read()` buffer. `Some` strictly for the duration of one
	/// `poll`; installed immediately before, cleared immediately after.
	buf: Cell<Option<NonNull<[u8]>>>,
	/// Write cursor into the installed buffer. Reset before each poll.
	pos: Cell<usize>,
	/// Set by `Writer` when *it* suspends. Distinguishes our own yield
	/// from a foreign future's `Pending`.
	yielded: Cell<bool>,
}

impl Shared {
	/// Copy as much of `src` as fits into the installed buffer.
	fn copy_in(&self, src: &[u8]) -> usize {
		let Some(dst) = self.buf.get() else { return 0 };
		let pos = self.pos.get();
		let n = src.len().min(dst.len() - pos);
		if n > 0 {
			// SAFETY:
			// * `dst` was derived (with whole-slice provenance) from the
			//   live `&mut [u8]` passed to the in-progress `read()` call,
			//   which the driver does not touch again until `poll`
			//   returns — and `copy_in` only runs inside `poll`.
			// * `src` derives from a `&[u8]`; `dst` from a `&mut [u8]`.
			//   `&mut` uniqueness rules out overlap, so
			//   `copy_nonoverlapping` is sound.
			unsafe {
				ptr::copy_nonoverlapping(src.as_ptr(), dst.cast::<u8>().as_ptr().add(pos), n);
			}
			self.pos.set(pos + n);
		}
		n
	}
}

/// Suspends exactly once, marking the suspension as ours.
struct YieldToReader<'a> {
	shared: &'a Shared,
	polled: bool,
}

impl Future for YieldToReader<'_> {
	type Output = ();

	fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
		if self.polled {
			Poll::Ready(())
		} else {
			self.polled = true;
			self.shared.yielded.set(true);
			Poll::Pending
		}
	}
}

/// Handle passed by value into the async body.
pub struct Writer {
	shared: Rc<Shared>,
}

impl Writer {
	/// Write all of `src` into the stream. Infallible `write_all`
	/// semantics: suspends (returning control to the reader) as many
	/// times as needed until every byte has been consumed.
	pub async fn write(&mut self, mut src: &[u8]) {
		loop {
			let n = self.shared.copy_in(src);
			src = &src[n..];
			if src.is_empty() {
				return;
			}
			// Caller's buffer is full; hand it back and wait for the
			// next `read()` to install a fresh one.
			YieldToReader {
				shared: &self.shared,
				polled: false,
			}
			.await;
		}
	}
}

/// The `Read` half. Created by [`async_read_generator`].
pub struct AsyncReadGen<Fut> {
	fut: Option<Pin<Box<Fut>>>, // dropped eagerly on completion
	shared: Rc<Shared>,
}

impl<Fut: Future<Output = ()>> io::Read for AsyncReadGen<Fut> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let Some(fut) = self.fut.as_mut() else {
			return Ok(0); // finished
		};
		if buf.is_empty() {
			// Must not poll: the writer could never make progress and
			// would yield, and we'd report Ok(0) — a false EOF.
			return Ok(0);
		}

		// Fresh reborrow => raw pointer with provenance over the whole
		// slice. The parent `&mut` is not used again until after `poll`
		// returns and the pointer is cleared.
		self.shared.buf.set(Some(NonNull::from(&mut *buf)));
		self.shared.pos.set(0);
		self.shared.yielded.set(false);

		let mut cx = Context::from_waker(Waker::noop());
		let poll = fut.as_mut().poll(&mut cx);

		self.shared.buf.set(None);
		let n = self.shared.pos.get();

		match poll {
			Poll::Ready(()) => {
				self.fut = None;
				Ok(n)
			}
			Poll::Pending if self.shared.yielded.get() => Ok(n),
			Poll::Pending => {
				panic!(
					"async_read_generator: body awaited a foreign future; this executor has no \
					 waker and cannot drive it"
				)
			}
		}
	}
}

/// Adapt an async writer body into a synchronous `io::Read`.
///
/// The body must only suspend via [`Writer::write`]. Awaiting anything
/// else that returns `Pending` — including yield-once futures à la
/// `yield_now()` — panics, deliberately: with a no-op waker we cannot
/// distinguish "poll me again" from "waiting on I/O", and re-polling
/// the latter is a livelock.
pub fn async_read_generator<F, Fut>(f: F) -> AsyncReadGen<Fut>
where
	F: FnOnce(Writer) -> Fut,
	Fut: Future<Output = ()>,
{
	let shared = Rc::new(Shared {
		buf: Cell::new(None),
		pos: Cell::new(0),
		yielded: Cell::new(false),
	});
	let writer = Writer {
		shared: Rc::clone(&shared),
	};
	AsyncReadGen {
		fut: Some(Box::pin(f(writer))),
		shared,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty() {
		let mut out = vec![];
		std::io::copy(&mut async_read_generator(async |_| {}), &mut out).unwrap();
		assert_eq!(out.len(), 0);
	}

	#[test]
	fn single_write() {
		let mut out = vec![];
		std::io::copy(
			&mut async_read_generator(async |mut w| {
				w.write(b"hello").await;
			}),
			&mut out,
		)
		.unwrap();

		assert_eq!(&out, b"hello");
	}

	#[test]
	fn multiple_writes() {
		let mut out = vec![];
		std::io::copy(
			&mut async_read_generator(async |mut w| {
				w.write(b"hello").await;
				w.write(b",").await;
				w.write(b" ").await;
				w.write(b"world").await;
				w.write(b"!").await;
			}),
			&mut out,
		)
		.unwrap();

		assert_eq!(&out, b"hello, world!");
	}

	#[test]
	fn empty_write() {
		let mut out = vec![];
		std::io::copy(
			&mut async_read_generator(async |mut w| {
				w.write(b"").await;
				w.write(b"a").await;
				w.write(b"").await;
				w.write(b"b").await;
				w.write(b"").await;
				w.write(b"c").await;
				w.write(b"").await;
			}),
			&mut out,
		)
		.unwrap();

		assert_eq!(&out, b"abc");
	}
}
