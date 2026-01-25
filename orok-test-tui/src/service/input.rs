use std::sync::Arc;

use anyhow::{Context, Result, bail};
use crossterm::event::{
	Event::*, EventStream, KeyCode, KeyCode::*, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
	MouseEventKind,
};
use tokio::sync::mpsc::Receiver;
use tokio_stream::StreamExt;

use super::tui::ui::MouseArea;

pub enum Event {
	SetMouseAreas { areas: Vec<MouseArea> },
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Focus {
	#[default]
	Gdb,
	Qemu,
	Nvim,
}

pub async fn run(bus: Arc<crate::Bus>, mut rx: Receiver<Event>) -> Result<!> {
	let mut stream = EventStream::new();

	let mut mouse_areas = Vec::new();

	#[expect(clippy::type_complexity)]
	let mut dragging: Option<(
		Arc<dyn Fn(&MouseEvent, Arc<crate::Bus>) -> bool + Send + Sync>,
		MouseButton,
	)> = None;

	let mut focus = Focus::default();

	let mut exit_attempts = 0;

	loop {
		tokio::select! {
			evt = stream.next() => {
				let evt = match evt {
					Some(Ok(evt)) => evt,
					Some(Err(err)) => bail!("input stream error: {err}"),
					None => bail!("EOF"),
				};

				match evt {
					Resize(_, _) => {
						bus.tui
							.send(crate::service::tui::Event::Invalidate)
							.await
							.with_context(|| "failed to send tui::Event::Invalidate")?;
					}

					Mouse(ev) if let Some((handler, button)) = dragging.as_mut() => {
						if let MouseEventKind::Drag(b) = ev.kind && b == *button {
							if handler(&ev, Arc::clone(&bus)) {
								bus.tui
									.send(crate::service::tui::Event::Invalidate)
									.await
									.with_context(|| "failed to send tui::Event::Invalidate")?;
							}
						} else {
							dragging = None;
						}
					}

					Key(KeyEvent {
						code: Char('d'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::CONTROL) => {
						match exit_attempts {
							0 => {
								bus.session
									.send(crate::service::session::Event::ScramAndQuit { status_code: 0 })
									.await
									.with_context(|| "failed to send session::Event::ScramAndQuit")?;
							}
							1 => {
								bus.app.send(crate::AppEvent::Quit { status_code: 1 })
									.await
									.with_context(|| "failed to send AppEvent::Quit")?;
							}
							_ => {
								panic!("failed to shut down gracefully; exiting hard");
							}
						}

						exit_attempts += 1;
					}

					Key(KeyEvent {
						code: Char('q'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::CONTROL) => {
						bus.tui
							.send(crate::service::tui::Event::ShowMouseAreas { show: None })
							.await
							.with_context(|| "failed to send tui::Event::ShowMouseAreas")?;
					}

					Key(KeyEvent {
						code: Char('l'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::CONTROL) => {
						bus.tui
							.send(crate::service::tui::Event::FullInvalidate)
							.await
							.with_context(|| "failed to send tui::Event::FullInvalidate")?;

						// We also send it to the TUIs
						bus.gdb
							.send(crate::service::gdb::Event::Stdin { bytes: vec![ 0x0C ] })
							.await
							.with_context(|| "failed to send ^L to gdb")?;
						bus.qemu
							.send(crate::service::qemu::Event::Stdin { bytes: vec![ 0x0C ] })
							.await
							.with_context(|| "failed to send ^L to qemu")?;
					}

					Key(KeyEvent {
						code: Char('g'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::ALT) => {
						focus = Focus::Gdb;
						bus.tui
							.send(crate::service::tui::Event::SetFocus { focus })
							.await
							.with_context(|| "failed to send tui::Event::SetFocus")?;
					}

					Key(KeyEvent {
						code: Char('q'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::ALT) => {
						focus = Focus::Qemu;
						bus.tui
							.send(crate::service::tui::Event::SetFocus { focus })
							.await
							.with_context(|| "failed to send tui::Event::SetFocus")?;
					}

					Key(KeyEvent {
						code: Char('c'), modifiers, ..
					}) if modifiers.contains(KeyModifiers::ALT) => {
						focus = Focus::Nvim;
						bus.tui
							.send(crate::service::tui::Event::SetFocus { focus })
							.await
							.with_context(|| "failed to send tui::Event::SetFocus")?;
					}

					Mouse(evt @ MouseEvent {
						kind,
						column,
						row,
						..
					}) => {
						let invalidate = mouse_areas
							.iter()
							.find(|area| {
								match (kind, area) {
									(
										MouseEventKind::Down(MouseButton::Left)
										| MouseEventKind::Drag(MouseButton::Left),
										MouseArea::LeftClick { rect, .. }
										| MouseArea::LeftClickDrag { rect, .. },
									) => rect.contains((column, row).into()),
									_ => false,
								}
							})
							.map(|area| {
								match area {
									MouseArea::LeftClick { handler, .. } => {
										handler(&evt, Arc::clone(&bus))
									}
									MouseArea::LeftClickDrag { handler, .. } => {
										let inv = handler(&evt, Arc::clone(&bus));
										dragging = Some((Arc::clone(handler), MouseButton::Left));
										inv
									}
								}
							})
							.unwrap_or(false);

						if invalidate {
							bus.tui
								.send(crate::service::tui::Event::Invalidate)
								.await
								.with_context(|| "failed to send tui::Event::Invalidate")?;
						}
					}

					Key(KeyEvent { code, modifiers, .. }) => 'skip_key: {
						let bytes = match code {
							KeyCode::Char(c) => {
								if modifiers.contains(KeyModifiers::CONTROL) {
									// Send control characters (but not Ctrl+D, Ctrl+G, Ctrl+L which we handle above)
									let ctrl_char = (c.to_ascii_uppercase() as u8) & 0x1F;
									vec![ctrl_char]
								} else {
									c.to_string().into_bytes()
								}
							}
							KeyCode::Enter => vec![b'\r'],
							KeyCode::Backspace => vec![0x7F],
							KeyCode::Esc => vec![0x1B],
							KeyCode::Up => b"\x1b[A".to_vec(),
							KeyCode::Down => b"\x1b[B".to_vec(),
							KeyCode::Right => b"\x1b[C".to_vec(),
							KeyCode::Left => b"\x1b[D".to_vec(),
							KeyCode::Home => b"\x1b[H".to_vec(),
							KeyCode::End => b"\x1b[F".to_vec(),
							KeyCode::PageUp => b"\x1b[5~".to_vec(),
							KeyCode::PageDown => b"\x1b[6~".to_vec(),
							KeyCode::Tab => vec![b'\t'],
							KeyCode::BackTab => b"\x1b[Z".to_vec(),
							KeyCode::Delete => b"\x1b[3~".to_vec(),
							KeyCode::Insert => b"\x1b[2~".to_vec(),
							KeyCode::F(n) if (1..=12).contains(&n) => {
								match n {
									1 => b"\x1bOP".to_vec(),
									2 => b"\x1bOQ".to_vec(),
									3 => b"\x1bOR".to_vec(),
									4 => b"\x1bOS".to_vec(),
									5 => b"\x1b[15~".to_vec(),
									6 => b"\x1b[17~".to_vec(),
									7 => b"\x1b[18~".to_vec(),
									8 => b"\x1b[19~".to_vec(),
									9 => b"\x1b[20~".to_vec(),
									10 => b"\x1b[21~".to_vec(),
									11 => b"\x1b[23~".to_vec(),
									12 => b"\x1b[24~".to_vec(),
									_ => unreachable!(),
								}
							}
							_ => break 'skip_key,
						};

						match focus {
							Focus::Gdb => {
								bus.gdb
									.send(crate::service::gdb::Event::Stdin { bytes })
									.await
									.with_context(|| "failed to send gdb::Event::Stdin")?;
							}
							Focus::Qemu => {
								bus.qemu
									.send(crate::service::qemu::Event::Stdin { bytes })
									.await
									.with_context(|| "failed to send qemu::Event::Stdin")?;
							}
							Focus::Nvim => {
								bus.nvim
									.send(crate::service::nvim::Event::Stdin { bytes })
									.await
									.with_context(|| "failed to send nvim::Event::Stdin")?;
							}
						}
					}
					_ => {}
				}
			}

			evt = rx.recv() => {
				let evt = match evt {
					Some(evt) => evt,
					None => bail!("EOF"),
				};

				match evt {
					Event::SetMouseAreas { areas } => {
						mouse_areas = areas;
					}
				}
			}
		}
	}
}
