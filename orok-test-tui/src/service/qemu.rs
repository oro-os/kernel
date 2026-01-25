use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use futures::pin_mut;
use portable_pty::CommandBuilder;
use tokio::sync::mpsc::Receiver;

use crate::pty::RunRequest;

pub enum Event {
	Scram,
	Start { args: Vec<String> },
	Stdin { bytes: Vec<u8> },
	Resize { rows: u16, cols: u16 },
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	rsp_path: impl AsRef<Path>,
	video_path: impl AsRef<Path>,
	orodbg_path: impl AsRef<Path>,
) -> Result<!> {
	let mut pending_run = None;
	let mut size = (24, 80);
	loop {
		let args = if let Some(args) = pending_run.take() {
			args
		} else {
			let Some(ev) = rx.recv().await else {
				bail!("EOF");
			};
			match ev {
				Event::Start { args } => args,
				Event::Scram | Event::Stdin { .. } => {
					// Ignore, we're not running
					continue;
				}
				Event::Resize { rows, cols } => {
					size = (rows, cols);
					// Ignore, we're not running
					continue;
				}
			}
		};

		log::debug!(
			"got request to start qemu ({:?}x{:?}) with args: {:?}",
			size.0,
			size.1,
			args
		);

		let mut command = CommandBuilder::new("make");
		command.args(&args);
		command.arg(format!(
			"QEMU_ARGS=-chardev socket,id=orovideo,path={} -chardev socket,path={},id=gdb0 -gdb \
			 chardev:gdb0 -S -chardev socket,path={},id=orokdbg",
			video_path.as_ref().display(),
			rsp_path.as_ref().display(),
			orodbg_path.as_ref().display(),
		));
		command.arg("ORO_TEST_MMIO=1");

		let child = match crate::pty::spawn(command, size.0, size.1).await {
			Ok(child) => child,
			Err(e) => {
				log::warn!("failed to spawn qemu: {e}");
				continue;
			}
		};

		if let Err(err) = bus.tui.send(crate::service::tui::Event::QemuStarted).await {
			log::warn!("failed to send QemuStarted event to tui: {err}");
		}

		let (request_sender, request_receiver) = tokio::sync::mpsc::channel(16);
		let (event_sender, mut event_receiver) = tokio::sync::mpsc::channel(16);

		// QEMU has spawned, now handle its I/O
		let code_fut = crate::pty::run_process(child, event_sender, request_receiver);

		pin_mut!(code_fut);

		let code = loop {
			tokio::select! {
				maybe_request = rx.recv() => {
					let Some(request) = maybe_request else {
						bail!("EOF");
					};

					match request {
						Event::Start { args } => {
							log::debug!("got qemu start request while an existing qemu session is active; restarting");
							// First, request shutdown of the existing session
							if let Err(e) = request_sender.send(RunRequest::Shutdown).await {
								log::warn!("failed to request qemu shutdown: {e}");
							}
							// Then, save the new request to be processed after the existing session ends
							pending_run = Some(args);
							break None;
						}
						Event::Stdin { bytes } => {
							if let Err(e) = request_sender.send(RunRequest::Stdin { bytes }).await {
								log::warn!("failed to send stdin to qemu pty: {e}");
								break None;
							}
						}
						Event::Resize { rows, cols } => {
							size = (rows, cols);
							if let Err(e) = request_sender.send(RunRequest::Resize { rows, cols }).await {
								log::warn!("failed to send resize to qemu pty: {e}");
								break None;
							}
						}
						Event::Scram => {
							log::debug!("got request to shutdown qemu session");
							if let Err(e) = request_sender.send(RunRequest::Shutdown).await {
								log::warn!("failed to send qemu shutdown request: {e}");
							}

							// NOTE: Don't break. We let the qemu process exit naturally.
						}

					}
				}

				maybe_event = event_receiver.recv() => {
					let Some(event) = maybe_event else {
						log::warn!("qemu PTY event channel closed unexpectedly");
						break None;
					};
					match event {
						crate::pty::RunEvent::Online => {
							// Nothing to do
						}
						crate::pty::RunEvent::Stdout { bytes } => {
							bus.tui.send(crate::service::tui::Event::QemuStdout { bytes }).await.with_context(|| "failed to send QemuStdout event to tui")?;
						}
					}
				}

				code = &mut code_fut => {
					break match code {
						Ok(code) => {
						log::debug!("qemu process exited with code: {code:?}");
						break code;
					} Err(e) => {
						log::error!("qemu process task panicked: {e}");
						None
					}}
				}
			}
		};

		log::info!("qemu session exited (code: {code:?})")
	}
}
