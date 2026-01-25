use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc::Receiver;

pub enum Event {}

pub async fn run(bus: Arc<crate::Bus>, rx: Receiver<Event>) -> Result<!> {
	drop(rx); // nobody should be sending anything to it.

	loop {
		crate::logging::wait_for_log().await;
		bus.tui
			.send(crate::service::tui::Event::Invalidate)
			.await
			.with_context(|| "failed to send TUI Invalidate event")?;
	}
}
