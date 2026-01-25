use std::{sync::Arc, time::Duration};

use anyhow::Result;
use tokio::sync::{Notify, mpsc::Receiver};

const DEBOUNCE_TIME: Duration = Duration::from_millis(100);
static NOTIFIER: Notify = Notify::const_new();

#[inline]
pub fn invalidate() {
	NOTIFIER.notify_one();
}

pub enum Event {}

pub async fn run(bus: Arc<crate::Bus>, rx: Receiver<Event>) -> Result<!> {
	// Don't let anyone send messages here.
	drop(rx);

	loop {
		NOTIFIER.notified().await;
		bus.tui.send(crate::service::tui::Event::Invalidate).await?;
		tokio::time::sleep(DEBOUNCE_TIME).await;
	}
}
