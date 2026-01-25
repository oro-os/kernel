use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use futures::pin_mut;
use portable_pty::CommandBuilder;
use tokio::sync::mpsc::Receiver;

use crate::pty::RunRequest;

pub enum Event {
	Stdin { bytes: Vec<u8> },
	Resize { rows: u16, cols: u16 },
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	nvim_path: impl AsRef<Path>,
	nvim_config_path: impl AsRef<Path>,
) -> Result<!> {
	// First, write the config
	tokio::fs::write(nvim_config_path.as_ref(), include_str!("./config.vim"))
		.await
		.with_context(|| "failed to write nvim config")?;

	let mut command = CommandBuilder::new("nvim");
	command.arg("-n");
	command.arg("--noplugin");
	command.arg("-u");
	command.arg(format!("{}", nvim_config_path.as_ref().display()));
	command.arg("--listen");
	command.arg(format!("{}", nvim_path.as_ref().display()));

	let child = crate::pty::spawn(command, 24, 80)
		.await
		.with_context(|| "failed to start `nvim`")?;

	let (request_sender, request_receiver) = tokio::sync::mpsc::channel(16);
	let (event_sender, mut event_receiver) = tokio::sync::mpsc::channel(16);

	let code_fut = crate::pty::run_process(child, event_sender, request_receiver);

	pin_mut!(code_fut);

	loop {
		tokio::select! {
			maybe_request = rx.recv() => {
				let Some(request) = maybe_request else {
					bail!("EOF");
				};

				match request {
					Event::Stdin { bytes } => {
						request_sender.send(RunRequest::Stdin { bytes }).await.with_context(||
							"failed to send stdin to nvim pty")?;
					}
					Event::Resize { rows, cols } => {
						request_sender.send(RunRequest::Resize { rows, cols }).await.with_context(||
							"failed to send resize to nvim pty")?;
					}
				}
			}

			maybe_event = event_receiver.recv() => {
				let Some(event) = maybe_event else {
					bail!("nvim event channel exited unexpectedly");
				};

				match event {
					crate::pty::RunEvent::Online => {
						bus.nvim_rpc.send(crate::service::nvim_rpc::Event::Ready).await
							.with_context(|| "failed to send Ready event to nvim_rpc")?;
					}
					crate::pty::RunEvent::Stdout { bytes } => {
						bus.tui.send(crate::service::tui::Event::NvimStdout { bytes }).await
							.with_context(|| "failed to send NvimStdout event to tui")?;
					}
				}
			}

			code = &mut code_fut => {
				match code {
					Ok(code) => {
					bail!("nvim exited with code {code:?}");
				} Err(e) => {
					bail!("nvim process task panicked; {e}");
				}}
			}
		}
	}
}
