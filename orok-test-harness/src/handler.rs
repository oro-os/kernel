//! Traits that receive the raw packets decoded from an event stream,
//! without any filtering or processing. These are used by the test harness
//! to receive packets from the stream and perform filtering and processing on them.

use crate::Packet;

/// When event streams are taken in by [`process_event_stream()`],
/// raw [`Packet`]s are emitted here.
pub trait RawPacketHandler {
	/// The error type returned by [`RawPacketHandler::handle_packet`].
	type Error: core::fmt::Debug;

	/// A raw event was emitted from the stream.
	///
	/// No filtering is performed here.
	fn handle_packet(&self, packet: Packet) -> impl Future<Output = Result<(), Self::Error>>;
}

#[expect(clippy::absolute_paths, reason = "one-off")]
impl RawPacketHandler for tokio::sync::mpsc::Sender<Packet> {
	type Error = tokio::sync::mpsc::error::SendError<Packet>;

	fn handle_packet(&self, packet: Packet) -> impl Future<Output = Result<(), Self::Error>> {
		self.send(packet)
	}
}
