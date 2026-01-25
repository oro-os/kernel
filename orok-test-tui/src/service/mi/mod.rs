mod types;

use std::{
	collections::HashMap,
	os::fd::FromRawFd,
	path::{Path, PathBuf},
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
};

use anyhow::{Context, Result, bail};
use futures::pin_mut;
use serde_gdbmi::SimpleResponseBody;
use tokio::{
	io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt},
	sync::{
		Mutex,
		mpsc::{Receiver, Sender},
		oneshot,
	},
};

#[derive(Debug)]
pub enum Event {
	Scram,
	Start,
	/// A MI command. Use `MiCommand::Foo{..}.into()` for easier conversion.
	MiCommand(Mi),
}

impl From<Mi> for Event {
	#[inline]
	fn from(mi: Mi) -> Self {
		Self::MiCommand(mi)
	}
}

#[derive(Debug)]
pub enum Mi {
	/// Sets the currently running executable, loading
	/// its symbols.
	SetFile { filepath: PathBuf },
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	mi_path: impl AsRef<Path>,
) -> Result<!> {
	loop {
		tokio::select! {
			res = rx.recv() => {
				let Some(evt) = res else {
					bail!("EOF");
				};

				match evt {
					Event::Start => {
						log::debug!("creating PTY session for GDB MI");
						let mut master: libc::c_int = 0;
						let mut slave: libc::c_int = 0;

						// SAFETY: This is safe, just calls via FFI.
						if unsafe {
							libc::openpty(
								&mut master,
								&mut slave,
								std::ptr::null_mut(),
								std::ptr::null(),
								std::ptr::null(),
							)
						} != 0
						{
							log::error!("failed to open pty for gdb MI interface");
							continue;
						}

						let master_fd = CloseOnDrop(master);
						let slave_fd = CloseOnDrop(slave);

						// Get the slave PTY path for GDB
						let mut buf = [0u8; 256];
						// SAFETY: This is safe, just calls via FFI.
						let dev_pty_path =
							if unsafe { libc::ttyname_r(slave_fd.0, buf.as_mut_ptr() as *mut i8, buf.len()) } == 0 {
								// SAFETY: Barring a bug in `ttyname_r`, this is safe.
								unsafe {
									std::ffi::CStr::from_ptr(buf.as_ptr() as *const i8)
										.to_string_lossy()
										.to_string()
								}
							} else {
								log::error!("failed to get slave pty name for gdb MI interface");
								continue;
							};

						// Symlink to the mipath
						log::trace!("attempting to unlink the existing symlink");
						if let Err(err) = tokio::fs::remove_file(mi_path.as_ref()).await && err.kind() != std::io::ErrorKind::NotFound {
							log::error!("failed to unlink existing MI symlink; GDB will likely fail: {err}: {}", mi_path.as_ref().display());
						}

						log::trace!("symlinking {} -> {}", mi_path.as_ref().display(), dev_pty_path);
						if let Err(err) = tokio::fs::symlink(&dev_pty_path, mi_path.as_ref()).await {
							log::error!("failed to symlink MI: {err}: {} -> {}", mi_path.as_ref().display(), dev_pty_path);
							continue;
						}

						log::debug!("GDB MI PTY created at {} -> {}", dev_pty_path, mi_path.as_ref().display());

						// SAFETY: We have validated that it's a real FD.
						let mi_fd = unsafe { std::fs::File::from_raw_fd(master_fd.consume()) };
						let mi_writer = tokio::fs::File::from_std(mi_fd);
						let mi_reader = match mi_writer.try_clone().await {
							Ok(r) => r,
							Err(e) => {
								log::error!("failed to clone gdb MI PTY fd: {e}");
								continue;
							}
						};

						let (mi_async_tx, mut mi_async_rx) = tokio::sync::mpsc::channel(64);

						let mi_session = MiSession::new(mi_writer, mi_async_tx);
						let mi_session_fut = mi_session.run(mi_reader);

						pin_mut!(mi_session_fut);

						loop {
							tokio::select! {
								res = rx.recv() => {
									let Some(evt) = res else {
										bail!("EOF");
									};

									match evt {
										Event::Scram => {
											log::debug!("received SCRAM; shutting down MI PTY");
											break;
										}
										Event::Start => {
											log::debug!("received new start request; restarting MI PTY");
											break;
										}
										Event::MiCommand(Mi::SetFile { filepath }) => {
											log::debug!("setting the current executable to: {}", filepath.display());
											match mi_session.send_command(format!("file-exec-and-symbols {}", filepath.display())).await {
												Ok(types::MiResponse::Done) => {
													log::info!("current executable set to: {}", filepath.display());
												}
												Ok(other) => {
													log::warn!("unexpected MI response when setting file to '{}': {other:?}", filepath.display());
												}
												Err(err) => {
													log::error!("failed to set current executable to '{}': {err:?}", filepath.display());
													break;
												}
											}
										}
									}
								}

								res = mi_async_rx.recv() => {
									let Some(evt) = res else {
										log::error!("MI async channel receiver closed unexpectedly");
										break;
									};

									match evt {
										types::MiResponse::Stopped { reason, frame, .. } => {
											let (file, line) = if let Some(frame) = frame {
												log::info!("kernel has stopped ({reason}) @ {}", frame.addr);
												(frame.fullname, frame.line)
											} else {
												log::info!("kernel has stopped ({reason})");
												(None, None)
											};

											bus.nvim_rpc.send(crate::service::nvim_rpc::Event::CloseAuxFile).await
												.with_context(|| "failed to send aux file closure to nvim_rpc service")?;

											if let Some(file) = file {
												bus.nvim_rpc
													.send(crate::service::nvim_rpc::Event::OpenMainFile { filename: PathBuf::from(file) })
													.await
													.with_context(|| "failed to send main file event to nvim_rpc service")?;
											}

											bus.nvim_rpc.send(crate::service::nvim_rpc::Event::SetMainHighlight { line }).await
												.with_context(|| "failed to send main file highlight to nvim_rpc service")?;
										}
										types::MiResponse::ThreadSelected { frame, .. } => {
											match frame.level {
												None | Some(0) => {
													bus.nvim_rpc.send(crate::service::nvim_rpc::Event::CloseAuxFile).await
														.with_context(|| "failed to send aux file closure to nvim_rpc service")?;
												}
												Some(_) => {
													if let Some(file) = frame.fullname {
														bus.nvim_rpc
																.send(crate::service::nvim_rpc::Event::OpenAuxFile { filename: PathBuf::from(file) })
																.await
																.with_context(|| "failed to send aux file event to nvim_rpc service")?;
													}

													bus.nvim_rpc.send(crate::service::nvim_rpc::Event::SetAuxHighlight { line: frame.line }).await
														.with_context(|| "failed to send aux file highlight to nvim_rpc service")?;
												}
											}
										}
										_=> {
											log::debug!("ignoring async event: {evt:?}");
										}
									}
								}

								res = &mut mi_session_fut => {
									let Err(err) = res;
									log::error!("MI session reader exited unexpectedly: {err}");
									break;
								}
							}
						}

						log::debug!("GDB MI session has shut down");
					}
					evt @ (Event::Scram | Event::MiCommand(_)) => {
						log::trace!("ignoring MI event (inactive MI session): {evt:?}");
					}
				}
			}
		}
	}
}
struct MiSession<W> {
	writer:      Mutex<W>,
	counter:     AtomicU64,
	token_queue: Mutex<HashMap<u64, oneshot::Sender<types::MiResponse>>>,
	async_tx:    Sender<types::MiResponse>,
}

impl<W> MiSession<W>
where
	W: AsyncWrite + Unpin,
{
	fn new(writer: W, async_tx: Sender<types::MiResponse>) -> Self {
		Self {
			writer: Mutex::new(writer),
			counter: AtomicU64::new(1),
			token_queue: Mutex::new(HashMap::new()),
			async_tx,
		}
	}

	async fn send_command(&self, command: impl AsRef<str>) -> Result<types::MiResponse> {
		// Create a unique token
		let token = self.counter.fetch_add(1, Ordering::SeqCst);
		let (resp_sender, resp_receiver) = oneshot::channel();
		self.token_queue.lock().await.insert(token, resp_sender);
		let token_str = token.to_string();

		// Send the command
		{
			let mut writer = self.writer.lock().await;
			writer
				.write_vectored(&[
					std::io::IoSlice::new(token_str.as_bytes()),
					std::io::IoSlice::new("-".as_bytes()),
					std::io::IoSlice::new(command.as_ref().as_bytes()),
					std::io::IoSlice::new(b"\n"),
				])
				.await
				.with_context(|| "failed to write command to mi channel")?;
			writer
				.flush()
				.await
				.with_context(|| "failed to flush mi channel")?;
		}

		// Wait for the reader to respond to us.
		let r = resp_receiver.await;
		self.token_queue.lock().await.remove(&token);

		// Respond!
		Ok(r?)
	}

	async fn run<R: AsyncRead + Unpin>(&self, reader: R) -> Result<!> {
		let mut buf_reader = tokio::io::BufReader::new(reader);
		let mut line = String::with_capacity(1024);
		loop {
			line.clear();
			let n = buf_reader
				.read_line(&mut line)
				.await
				.with_context(|| "failed to read line from mi channel")?;

			if n == 0 {
				bail!("EOF");
			}

			let line = line.trim_end();

			log::trace!("[mi] {line}");
			let resp = match serde_gdbmi::from_str(line) {
				Ok(resp) => resp,
				Err(e) => {
					log::warn!("failed to parse mi response: {e}: {line}");
					continue;
				}
			};

			match (resp.body.into_simple(), resp.token) {
				(None, _) => {
					// "(gdb)" prompt, ignore
					continue;
				}
				(Some(SimpleResponseBody::Stream(_s)), _) => {
					// Do nothing, we just trace it above and leave it be.
				}
				(Some(SimpleResponseBody::Data(d)), None) => {
					self.async_tx.send(d).await?;
				}
				(Some(SimpleResponseBody::Data(d)), Some(token_num)) => {
					let mut token_queue = self.token_queue.lock().await;
					if let Some(resp_sender) = token_queue.remove(&token_num) {
						if let Err(err) = resp_sender.send(d) {
							log::trace!("sender for token {token_num} was closed: {err:?}");
						}
					} else {
						log::warn!("received response for unknown token: {token_num}: {:?}", d);
					}
				}
			}
		}
	}
}

struct CloseOnDrop(libc::c_int);

impl CloseOnDrop {
	fn consume(self) -> libc::c_int {
		let fd = self.0;
		log::trace!("consuming fd: {}", self.0);
		core::mem::forget(self);
		fd
	}
}

impl Drop for CloseOnDrop {
	fn drop(&mut self) {
		// SAFETY: We assume this is safe; used only internally.
		unsafe {
			log::trace!("closing fd: {}", self.0);
			libc::close(self.0);
		}
	}
}
