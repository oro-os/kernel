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
	/// Sends a QEMU RSP packet to the connected client
	SendPacket { data: Vec<u8> },
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	socket_path: impl AsRef<Path>,
) -> Result<!> {
	let server = UnixListener::bind(&socket_path).with_context(|| {
		format!(
			"failed to bind to QEMU RSP socket at {:?}",
			socket_path.as_ref()
		)
	})?;

	let mut read_buf = vec![0u8; 4096];

	loop {
		log::debug!("waiting for QEMU RSP client");

		let Some(Either::Value((mut socket, _))) = server.accept().or_event(&mut rx).await? else {
			// Ignore incoming events.
			continue;
		};

		log::debug!("QEMU RSP client connected");

		bus.session
			.send(crate::service::session::Event::QemuConnected)
			.await
			.with_context(|| "failed to send QemuStarted event to session service")?;

		loop {
			match socket.read(&mut read_buf).or_event(&mut rx).await? {
				None => {
					log::debug!("QEMU RSP client disconnected");
					break;
				}
				Some(Either::Value(n)) => {
					if n == 0 {
						log::debug!("QEMU RSP client disconnected (EOF)");
						break;
					}

					if let Err(err) = bus
						.gdb_rsp
						.send(crate::service::gdb_rsp::Event::SendPacket {
							data: read_buf[..n].to_vec(),
						})
						.await
					{
						log::warn!("failed to send packet to GDB RSP service: {err}");
						break;
					}
				}
				Some(Either::Event(Event::SendPacket { data })) => {
					if let Err(err) = socket.write_all(&data).await {
						log::warn!("failed to send packet to QEMU RSP client: {err}");
						break;
					}
				}
				Some(Either::Event(Event::Scram)) => {
					log::debug!("received shutdown request for QEMU RSP client");
					break;
				}
			}
		}

		log::debug!("QEMU RSP client disconnected");
	}
}
