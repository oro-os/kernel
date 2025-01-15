//! Native threads.

mod current;

pub use self::current::{current, yield_now};
use crate::{fmt, num::NonZero};

/// A unique identifier for a running thread.
///
/// A `ThreadId` is an opaque object that uniquely identifies each thread created during
/// the lifetime of a process. `ThreadId`s are guaranteed not to be reused, even when a
/// thread terminates. `ThreadId`s are under the control of Rust’s standard library and there
/// may not be any relationship between `ThreadId` and the underlying platform’s notion
/// of a thread identifier – the two concepts cannot, therefore, be used interchangeably.
/// A `ThreadId` can be retrieved from the `id` method on a Thread.
#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct ThreadId(NonZero<u64>);

/// This returns a numeric identifier for the thread identified by this
/// `ThreadId`.
///
/// As noted in the documentation for the type itself, it is essentially an
/// opaque ID, but is guaranteed to be unique for each thread. The returned
/// value is entirely opaque -- only equality testing is stable. Note that
/// it is not guaranteed which values new threads will return, and this may
/// change across Rust versions.
impl ThreadId {
	/// This returns a numeric identifier for the thread identified by this
	/// `ThreadId`.
	///
	/// As noted in the documentation for the type itself, it is essentially an
	/// opaque ID, but is guaranteed to be unique for each thread. The returned
	/// value is entirely opaque -- only equality testing is stable. Note that
	/// it is not guaranteed which values new threads will return, and this may
	/// change across Rust versions.
	#[cfg(all(feature = "nightly", feature = "thread_id_value"))]
	#[must_use]
	pub fn as_u64(&self) -> NonZero<u64> {
		self.0
	}
}

/// A handle to a thread.
#[derive(Clone)]
pub struct Thread {
	id: ThreadId,
}

impl Thread {
	/// Internal method to create a new thread handle.
	#[must_use]
	pub(crate) fn new(id: NonZero<u64>) -> Self {
		Self { id: ThreadId(id) }
	}

	/// Gets the thread’s unique identifier.
	#[must_use]
	pub fn id(&self) -> ThreadId {
		self.id
	}

	/// Gets the thread's name.
	#[must_use]
	pub fn name(&self) -> Option<&str> {
		None
	}
}

impl fmt::Debug for Thread {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Thread")
			.field("id", &self.id())
			.field("name", &self.name())
			.finish_non_exhaustive()
	}
}
