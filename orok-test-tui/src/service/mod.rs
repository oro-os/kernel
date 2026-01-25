pub mod debounce;
pub mod gdb;
pub mod gdb_rsp;
pub mod input;
pub mod logging;
pub mod mi;
pub mod nvim;
pub mod nvim_rpc;
pub mod orodbg;
pub mod orodbg_sock;
pub mod qemu;
pub mod qemu_rsp;
pub mod session;
pub mod tui;
pub mod video;

pub enum Either<T, Event> {
	Value(T),
	Event(Event),
}

#[allow(unused)]
impl<T, Event> Either<T, Event> {
	pub fn is_value(&self) -> bool {
		matches!(self, Self::Value(_))
	}

	pub fn is_event(&self) -> bool {
		matches!(self, Self::Event(_))
	}

	pub fn map_value<U>(self, f: impl FnOnce(T) -> U) -> Either<U, Event> {
		match self {
			Self::Value(v) => Either::Value(f(v)),
			Self::Event(e) => Either::Event(e),
		}
	}

	pub fn map_event<F>(self, f: impl FnOnce(Event) -> F) -> Either<T, F> {
		match self {
			Self::Value(v) => Either::Value(v),
			Self::Event(e) => Either::Event(f(e)),
		}
	}

	pub fn unwrap_value(self) -> T {
		match self {
			Self::Value(v) => v,
			Self::Event(_) => panic!("called `Either::unwrap_value()` on an `Event` variant"),
		}
	}

	pub fn unwrap_event(self) -> Event {
		match self {
			Self::Value(_) => panic!("called `Either::unwrap_event()` on a `Value` variant"),
			Self::Event(e) => e,
		}
	}
}

pub trait FutExt<T, Event, Error: EofError>: Future<Output = Result<T, Error>> {
	async fn or_event(
		self,
		receiver: &mut tokio::sync::mpsc::Receiver<Event>,
	) -> Result<Option<Either<T, Event>>, Error>
	where
		Self: Sized,
	{
		tokio::select! {
			res = self => match res {
				Ok(v) => Ok(Some(Either::Value(v))),
				Err(e) if e.is_eof() => Ok(None),
				Err(e) => Err(e)
			},
			evt = receiver.recv() => match evt {
				Some(ev) => Ok(Some(Either::Event(ev))),
				None => Err(Error::eof())
			},
		}
	}
}

impl<F, T, Event, Error: EofError> FutExt<T, Event, Error> for F where
	F: Future<Output = Result<T, Error>>
{
}

pub trait EofError {
	fn is_eof(&self) -> bool;
	fn eof() -> Self
	where
		Self: Sized;
}

impl EofError for std::io::Error {
	fn is_eof(&self) -> bool {
		self.kind() == std::io::ErrorKind::UnexpectedEof
	}

	fn eof() -> Self {
		std::io::Error::new(
			std::io::ErrorKind::UnexpectedEof,
			"unexpected end of event stream",
		)
	}
}
