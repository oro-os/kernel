use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{Packet, RawPacketHandler};

/// An error returned by [`process_event_stream`].
#[derive(Debug, thiserror::Error)]
pub enum StreamError<E: core::fmt::Debug> {
	/// An error occurred within the [`RawPacketHandler`] during stream processing.
	#[error("event stream handler errored during stream processing: {0}")]
	Handler(E),
	/// An IO error occurred during stream processing
	#[error("IO error during stream processing: {0}")]
	Io(#[from] std::io::Error),
}

/// Handles an event stream asynchronously, emitting [`Packet`]s via a [`RawPacketHandler`].
///
/// Returns `Err` that holds either a stream error, or an error returned from [`RawPacketHandler::handle_packet`].
/// Otherwise, never returns.
pub async fn process_event_stream<H: RawPacketHandler>(
	mut sock: impl AsyncRead + Unpin,
	handler: H,
) -> Result<!, StreamError<H::Error>> {
	let mut packet = [0u64; 8];
	let mut buf = [[0u8; 8]; 7];
	loop {
		// Read the marker register, which tells us
		// the source of the event (QEMU vs kernel),
		// the event ID, and a bitmask of which registers
		// were transmitted (all others are set to 0).
		//
		// For the purposes of decoding, we're only interested
		// in the bitmask. The orodbg service will care
		// about the other bits.
		//
		// https://github.com/oro-os/oro-qemu/blob/master/scripts/oro-decode-kdbg.py
		sock.read_exact(&mut buf[0]).await?;
		let reg0 = u64::from_le_bytes(buf[0]);
		packet[0] = reg0;

		let bitmask = (reg0 >> 56) & 0x7F;
		let to_read = bitmask.count_ones() as usize;

		// SAFETY: This is just a performance hack,
		// SAFETY: as this is a hot path.
		sock.read_exact(unsafe {
			core::slice::from_raw_parts_mut(
				core::ptr::from_mut(&mut buf[..to_read]).cast(),
				to_read * 8,
			)
		})
		.await?;

		let mut read_i = 0;
		for i in 0..7 {
			let was_xfer = (bitmask & (1 << i)) > 0;
			if was_xfer {
				packet[i + 1] = u64::from_le_bytes(buf[read_i]);
				read_i += 1;
			} else {
				packet[i + 1] = 0;
			}
		}

		handler
			.handle_packet(Packet(packet))
			.await
			.map_err(StreamError::Handler)?;
	}
}
