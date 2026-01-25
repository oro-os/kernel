use std::{
	cell::RefCell,
	io::{Read, Write},
	os::fd::AsRawFd,
	pin::Pin,
	task::{Context, Poll, ready},
};

use futures::pin_mut;
use portable_pty::{CommandBuilder, PtySize};
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf, unix::AsyncFd},
	sync::mpsc::{Receiver, Sender},
};

pub struct Child {
	child:  Box<dyn portable_pty::Child + Send + Sync>,
	pair:   portable_pty::PtyPair,
	reader: FdReader,
	writer: FdWriter,
}

impl Drop for Child {
	fn drop(&mut self) {
		let _ = self.child.kill();
		for _ in 0..5 {
			if self.child.try_wait().is_ok() {
				return;
			}
			std::thread::sleep(std::time::Duration::from_millis(100));
		}
		log::warn!("gdb process did not exit in time; zombie process may remain");
	}
}

struct FdReaderHandle {
	fd:    i32,
	inner: Box<dyn Read + Send>,
}

impl AsRawFd for FdReaderHandle {
	fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
		self.fd
	}
}

pub struct FdReader {
	handle: AsyncFd<FdReaderHandle>,
}

impl FdReader {
	pub fn new(fd: i32, inner: Box<dyn Read + Send>) -> Self {
		Self {
			handle: AsyncFd::new(FdReaderHandle { fd, inner }).unwrap(),
		}
	}
}

impl AsyncRead for FdReader {
	fn poll_read(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<tokio::io::Result<()>> {
		loop {
			let mut guard = ready!(self.handle.poll_read_ready_mut(cx))?;

			let unfilled = buf.initialize_unfilled();
			match guard.try_io(|inner| inner.get_mut().inner.read(unfilled)) {
				Ok(Ok(len)) => {
					buf.advance(len);
					return Poll::Ready(Ok(()));
				}
				Ok(Err(err)) => return Poll::Ready(Err(err)),
				Err(_would_block) => continue,
			}
		}
	}
}

struct FdWriterHandle {
	fd:    i32,
	inner: Box<dyn Write + Send>,
}

impl AsRawFd for FdWriterHandle {
	fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
		self.fd
	}
}

pub struct FdWriter {
	handle: AsyncFd<FdWriterHandle>,
}

impl FdWriter {
	pub fn new(fd: i32, inner: Box<dyn Write + Send>) -> Self {
		Self {
			handle: AsyncFd::new(FdWriterHandle { fd, inner }).unwrap(),
		}
	}
}

impl AsyncWrite for FdWriter {
	fn poll_write(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<tokio::io::Result<usize>> {
		loop {
			let mut guard = ready!(self.handle.poll_write_ready_mut(cx))?;

			match guard.try_io(|inner| inner.get_mut().inner.write(buf)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
		loop {
			let mut guard = ready!(self.handle.poll_write_ready_mut(cx))?;

			match guard.try_io(|inner| inner.get_mut().inner.flush()) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
		// No special shutdown needed
		Poll::Ready(Ok(()))
	}
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("failed to open pty: {0}")]
	OpenPty(Box<dyn std::error::Error + Send + Sync>),
	#[error("failed to spawn command: {0}")]
	SpawnCommand(Box<dyn std::error::Error + Send + Sync>),
	#[error("failed to get pty master fd")]
	GetFd,
	#[error("failed to set pty master fd to non-blocking")]
	SetNonBlocking,
	#[error("failed to duplicate pty master fd for reading")]
	DupRead,
	#[error("failed to duplicate pty master fd for writing")]
	DupWrite,
	#[error("unexpected end of request stream")]
	Eof,
	#[error("failed to kill process: {0}")]
	Kill(std::io::Error),
	#[error("spawn task panicked: {0}")]
	SpawnPanic(tokio::task::JoinError),
	#[error("spawn task join error: {0}")]
	SpawnJoin(tokio::task::JoinError),
	#[error("failed to send event to event stream: {0}")]
	Send(#[from] tokio::sync::mpsc::error::SendError<RunEvent>),
	#[error("failed to read from pty: {0}")]
	Read(std::io::Error),
}

pub enum RunRequest {
	Shutdown,
	Resize { rows: u16, cols: u16 },
	Stdin { bytes: Vec<u8> },
}

pub enum RunEvent {
	Online,
	Stdout { bytes: Vec<u8> },
}

pub async fn run_process(
	mut child: Child,
	sender: Sender<RunEvent>,
	mut receiver: Receiver<RunRequest>,
) -> Result<Option<u32>, Error> {
	let Child {
		reader,
		writer,
		pair,
		child,
	} = &mut child;

	let mut killer = child.clone_killer();

	let child_ref = RefCell::new(&mut *child);

	let wait_fut = async {
		loop {
			let r = child_ref.borrow_mut().try_wait();
			match r {
				Ok(Some(status)) => break Ok(status.exit_code()),
				Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
				Err(e) => {
					log::warn!("error while waiting for gdb to exit: {e}");
					break Err(e);
				}
			}
		}
	};

	let read_fut = {
		let sender = sender.clone();
		async move {
			let mut buf = [0u8; 1024];
			loop {
				match reader.read(&mut buf).await {
					Ok(0) => {
						log::debug!("read 0 bytes from stdout; assuming closed");
						break Ok::<(), Error>(());
					}
					Ok(n) => {
						sender
							.send(RunEvent::Stdout {
								bytes: buf[..n].to_vec(),
							})
							.await
							.map_err(Error::Send)?;
					}
					Err(e) => {
						log::warn!("error reading from stdout: {e}");
						break Err(Error::Read(e));
					}
				}
			}
		}
	};

	pin_mut!(wait_fut);
	pin_mut!(read_fut);

	// Send ready event
	sender.send(RunEvent::Online).await?;

	let code = 'ret_code: loop {
		tokio::select! {
			biased;

			status = &mut wait_fut => {
				break status.ok();
			}

			res = &mut read_fut => {
				if let Err(e) = res {
					log::warn!("error reading from process: {e}");
				}

				// pty died; no exit code
				break None;
			}

			msg = receiver.recv() => {
				match msg {
					Some(RunRequest::Stdin { bytes }) => {
						if let Err(e) = <_ as AsyncWriteExt>::write_all(writer, &bytes).await {
							log::warn!("error writing to stdin: {e}");
							return Err(Error::Eof);
						}
					}
					Some(RunRequest::Resize { cols, rows }) => {
						if let Err(e) = pair.master.resize(PtySize {
							rows,
							cols,
							pixel_width: 0,
							pixel_height: 0,
						}) {
							log::warn!("error resizing pty: {e}");
						}
					}
					Some(RunRequest::Shutdown) => {
						log::debug!("received shutdown request");
						if let Err(e) = killer.kill().map_err(Error::Kill) {
							log::debug!("failed to kill process: {e}");
							return Err(e);
						}

						// Prevent the wait_fut from getting a mut ref to the child.
						// Keep the spaceship running, even if this is unnecessary.
						#[expect(clippy::drop_non_drop, reason = "keep the spaceship running")]
						{
							drop(wait_fut);
						}

						log::debug!("kill signal sent to process; waiting");
						for i in 0..30 {
							if let Ok(code) = child_ref.borrow_mut().try_wait() {
								log::debug!("process exited with code: {code:?}");
								break 'ret_code code.map(|c| c.exit_code());
							}
							log::debug!("waiting for process to exit... ({}/30)", i + 1);
							tokio::time::sleep(std::time::Duration::from_millis(100)).await;
						}
						log::error!("process did not exit in time after kill; it's definitely zombified");
						break 'ret_code None;
					}
					None => {
						log::debug!("request channel closed");
						return Err(Error::Eof);
					}
				}
			}
		}
	};

	log::debug!("shutting down process");
	Ok(code)
}

fn run_sync(mut command: CommandBuilder, rows: u16, cols: u16) -> Result<Child, Error> {
	// ---- REMINDER ----
	//
	// This function is synchronous. `log::info!` et al dispatch to the
	// *asynchronous* logging system. Thus, hangs, blocking, errors, etc.
	// might not immediately show up in the logger output, and be confusing
	// if debugging problematic behavior here.
	//
	// Be sure to check in a debugger where things are _really_ halted.

	let pty_system = portable_pty::native_pty_system();
	let pair = pty_system
		.openpty(PtySize {
			rows,
			cols,
			pixel_width: 0,
			pixel_height: 0,
		})
		.map_err(|e| Error::OpenPty(e.into()))?;

	if command.get_cwd().is_none() {
		match std::env::current_dir() {
			Ok(cwd) => {
				command.cwd(cwd);
			}
			Err(e) => {
				log::warn!(
					"failed to get current directory; spawned command might not run correctly: {e}"
				);
			}
		}
	}

	log::debug!("spawning command: {:?}", command.as_unix_command_line());

	let child = pair
		.slave
		.spawn_command(command)
		.map_err(|e| Error::SpawnCommand(e.into()))?;

	let pid = child.process_id();
	log::debug!("command spawned with pid {pid:?}");

	let fd = pair.master.as_raw_fd().ok_or(Error::GetFd)?;

	// SAFETY: We just got this fd from portable-pty; it should be valid.
	let (fr_w, fd_r) = unsafe {
		if libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) != 0 {
			return Err(Error::SetNonBlocking);
		}
		let fr = libc::dup(fd);
		if fr < 0 {
			return Err(Error::DupRead);
		}
		let fw = libc::dup(fd);
		if fw < 0 {
			libc::close(fr);
			return Err(Error::DupWrite);
		}

		(fw, fr)
	};

	let writer = pair.master.take_writer().unwrap();
	let reader = pair.master.try_clone_reader().unwrap();

	log::debug!("got command stdio handles");

	Ok(Child {
		child,
		pair,
		reader: FdReader::new(fd_r, reader),
		writer: FdWriter::new(fr_w, writer),
	})
}

pub async fn spawn(command: CommandBuilder, rows: u16, cols: u16) -> Result<Child, Error> {
	let maybe_child = tokio::task::spawn_blocking(move || run_sync(command, rows, cols));

	// The above was synchronous; let the runtime breathe
	// for a moment.
	tokio::task::yield_now().await;

	match maybe_child.await {
		Ok(Ok(child)) => Ok(child),
		Ok(Err(e)) => Err(e),
		Err(e) if e.is_panic() => Err(Error::SpawnPanic(e)),
		Err(e) => Err(Error::SpawnJoin(e)),
	}
}
