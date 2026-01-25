use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::UnixListener,
	sync::mpsc::Receiver,
};

use super::{Either, FutExt};

pub enum Event {
	/// Kills the connection immediately
	Scram,
	/// Sends a GDB RSP packet to the connected client
	SendPacket { data: Vec<u8> },
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	socket_path: impl AsRef<Path>,
) -> Result<!> {
	let server = UnixListener::bind(&socket_path).with_context(|| {
		format!(
			"failed to bind to GDB RSP socket at {:?}",
			socket_path.as_ref()
		)
	})?;

	let mut read_buf = vec![0u8; 4096];
	let mut decoder = gdb_protocol::parser::Parser::default();

	loop {
		log::debug!("waiting for GDB RSP client");

		let Some(Either::Value((mut socket, _))) = server.accept().or_event(&mut rx).await? else {
			// Ignore incoming events.
			continue;
		};

		log::debug!("GDB RSP client connected");

		loop {
			match socket.read(&mut read_buf).or_event(&mut rx).await? {
				None => {
					log::debug!("GDB RSP client disconnected");
					break;
				}
				Some(Either::Value(n)) => {
					if n == 0 {
						log::debug!("GDB RSP client disconnected (EOF)");
						break;
					}

					let mut offset = 0;
					while offset < n {
						match decoder.feed(&read_buf[offset..n]) {
							Ok((consumed, Some(packet))) => {
								if let Some(packet) = packet.clone().check() {
									log::trace!("[GDB RSP] {packet:?}");
								} else {
									log::trace!("[GDB RSP] {packet:?}");
								}
								offset += consumed;
							}
							Ok((consumed, None)) => {
								offset += consumed;
							}
							Err(e) => {
								log::warn!("failed to parse GDB RSP packet: {e}");
								break;
							}
						}
					}

					if let Err(err) = bus
						.qemu_rsp
						.send(crate::service::qemu_rsp::Event::SendPacket {
							data: read_buf[..n].to_vec(),
						})
						.await
					{
						log::warn!("failed to send packet to QEMU RSP service: {err}");
						break;
					}
				}
				Some(Either::Event(Event::SendPacket { data })) => {
					if let Err(err) = socket.write_all(&data).await {
						log::warn!("failed to send packet to GDB RSP client: {err}");
						break;
					}
				}
				Some(Either::Event(Event::Scram)) => {
					log::debug!("received shutdown request for GDB RSP client");
					break;
				}
			}
		}

		log::debug!("GDB RSP client disconnected");
	}
}
