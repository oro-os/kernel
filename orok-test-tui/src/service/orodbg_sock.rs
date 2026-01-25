use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use futures::pin_mut;
use tokio::{net::UnixListener, sync::mpsc::Receiver};

pub enum Event {
	Scram,
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	orodbg_path: impl AsRef<Path>,
) -> Result<!> {
	let listener =
		UnixListener::bind(orodbg_path).with_context(|| "failed to bind to orodbg path")?;

	loop {
		tokio::select! {
			res = rx.recv() => {
				let Some(evt) = res else {
					bail!("EOF");
				};

				match evt {
					Event::Scram => {
						// Nothing to do
					}
				}
			}

			sock = listener.accept() => {
				let (sock, _) = match sock {
					Ok(sock) => sock,
					Err(err) => {
						log::warn!("failed to accept orodbg socket: {err}");
						continue;
					}
				};

				log::debug!("orodbg connection received");

				let (packet_tx, mut packet_rx) = tokio::sync::mpsc::channel(512);

				let sock_fut = orok_test_harness::process_event_stream(sock, packet_tx);

				pin_mut!(sock_fut);

				loop {
					tokio::select! {
						biased;

						res = packet_rx.recv() => {
							let Some(packet) = res else {
								log::warn!("orodbg socket received EOF");
								break;
							};

							log::trace!("[orodbg] {packet:?}");
							bus.orodbg.send(crate::service::orodbg::Event::Packet { packet }).await
								.with_context(|| "failed to send to orodbg service")?;
						}

						res = &mut sock_fut => {
							let Err(err) = res;
							log::warn!("orodbg socket driver errored unexpectedly: {err}");
							break;
						}

						res = rx.recv() => {
							let Some(evt) = res else { bail!("EOF"); };
							match evt {
								Event::Scram => {
									log::debug!("received scram event; ending orodbg socket");
									break;
								}
							}
						}
					}
				}
			}
		}
	}
}
