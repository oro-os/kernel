use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage};
use tokio::{io::AsyncReadExt, net::UnixListener, sync::mpsc::Receiver};

use super::{Either, FutExt};

pub enum Event {
	Scram,
}

pub async fn run(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	path: impl AsRef<Path>,
) -> Result<!> {
	let server = UnixListener::bind(path.as_ref()).context("failed to bind to video socket")?;
	log::debug!("QEMU video service listening on {:?}", path.as_ref());

	loop {
		let mut last_size: (u64, u64) = (640, 480);

		bus.tui
			.send(crate::service::tui::Event::VideoDisconnected)
			.await
			.with_context(|| "failed to send VideoDisconnected TUI event")?;

		let Some(Either::Value((mut socket, _))) = server
			.accept()
			.or_event(&mut rx)
			.await
			.with_context(|| "failed to accept incoming video stream connection")?
		else {
			// Ignore scram
			continue;
		};

		bus.tui
			.send(crate::service::tui::Event::VideoConnected)
			.await
			.with_context(|| "failed to send VideoConnected TUI event")?;

		bus.tui
			.send(crate::service::tui::Event::VideoSizeChanged {
				width:  last_size.0,
				height: last_size.1,
			})
			.await
			.with_context(|| "failed to send VideoSizeChanged event TUI event")?;

		loop {
			let mut size_bytes = [0u8; 8];
			let Some(Either::Value(width)) = socket
				.read_exact(&mut size_bytes)
				.or_event(&mut rx)
				.await?
				.map(|v| v.map_value(|_| u64::from_ne_bytes(size_bytes)))
			else {
				log::debug!("video client disconnected (EOF while reading width)");
				break;
			};

			let Some(Either::Value(height)) = socket
				.read_exact(&mut size_bytes)
				.or_event(&mut rx)
				.await?
				.map(|v| v.map_value(|_| u64::from_ne_bytes(size_bytes)))
			else {
				log::debug!("video client disconnected (EOF while reading height)");
				break;
			};

			let total_size = width * height * 3;
			let Ok(total_size) = usize::try_from(total_size) else {
				log::error!(
					"received video frame with invalid size (doesn't fit in usize): \
					 {width}x{height}"
				);
				break;
			};

			if (width, height) != last_size {
				last_size = (width, height);
				bus.tui
					.send(crate::service::tui::Event::VideoSizeChanged { width, height })
					.await
					.with_context(|| "failed to send VideoSizeChanged event TUI event")?;
			}

			let Ok(width) = u32::try_from(width) else {
				log::error!(
					"received video frame with invalid width (doesn't fit in u32): {width}"
				);
				break;
			};

			let Ok(height) = u32::try_from(height) else {
				log::error!(
					"received video frame with invalid height (doesn't fit in u32): {height}"
				);
				break;
			};

			let Ok(mut frame_data) = Vec::<u8>::try_with_capacity(total_size) else {
				log::error!(
					"received video frame with invalid size (allocation failure): {width}x{height}"
				);
				break;
			};

			// SAFETY: We just allocated the buffer with the required capacity
			let Some(Either::Value(_)) = socket
				.read_exact(unsafe {
					core::slice::from_raw_parts_mut(
						frame_data.spare_capacity_mut().as_mut_ptr().cast(),
						total_size,
					)
				})
				.or_event(&mut rx)
				.await?
			else {
				log::debug!("video client disconnected (EOF while reading frame data)");
				break;
			};

			// SAFETY: We just filled the buffer with valid data
			unsafe {
				frame_data.set_len(total_size);
			}

			let img = RgbImage::from_vec(width, height, frame_data).unwrap();
			let img = DynamicImage::ImageRgb8(img);
			bus.tui
				.send(crate::service::tui::Event::VideoFrame { image: img })
				.await
				.with_context(|| "failed to send VideoFrame event TUI event")?;
		}

		log::debug!("video client disconnected");
	}
}
