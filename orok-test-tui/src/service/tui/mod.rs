pub mod rect;
pub mod ui;

use std::{cell::RefCell, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use ratatui::{
	Terminal,
	buffer::Buffer,
	layout::{Constraint, Direction, Layout, Rect, Spacing},
	style::{Color, Style},
	widgets::Widget,
};
use tokio::sync::mpsc::Receiver;

use self::rect::RectExt;
use crate::service::input::Focus;

pub enum Event {
	/// Invalidates the TUI (re-draws it)
	Invalidate,
	/// Perform a complete invalidation (clearing the entire screen first).
	/// Slow, use [`Event::Invalidate`] if possible.
	FullInvalidate,
	/// Sets whether or not the mouse areas should be shown.
	ShowMouseAreas {
		/// If `Some`, definitively sets whether or not to show the mouse areas. If `None`, toggles the current state.
		show: Option<bool>,
	},
	/// A new video frame is available. The TUI should redraw itself to show the new frame.
	VideoFrame { image: image::DynamicImage },
	/// The video client has disconnected. The TUI should stop showing the video frame.
	VideoDisconnected,
	/// The video client has connected. The TUI should prepare to show the video frame.
	VideoConnected,
	/// The video frame size has changed.
	/// new size.
	VideoSizeChanged { width: u64, height: u64 },
	/// GDB has started. The TUI should prepare to show the GDB interface.
	GdbStarted,
	/// GDB emitted some stdout.
	GdbStdout { bytes: Vec<u8> },
	/// QEMU has started. The TUI should prepare to show the QEMU output.
	QemuStarted,
	/// QEMU emitted some stdout.
	QemuStdout { bytes: Vec<u8> },
	/// Sets the currently focused item, according to the input service.
	SetFocus { focus: Focus },
	/// The GDB TUI should be resized to the given dimensions (in characters).
	///
	/// Note that this also forwards the event to the GDB service
	ResizeGdb { rows: u16, cols: u16 },
	/// The QEMU TUI should be resized to the given dimensions (in characters).
	///
	/// Note that this also forwards the event to the QEMU service
	ResizeQemu { rows: u16, cols: u16 },
	/// Sets the current orodbg debugstate handle.
	SetDebugState {
		debug_state: Arc<orok_test_harness::State>,
	},
	/// Nvim emitted stdout
	NvimStdout { bytes: Vec<u8> },
	/// Nvim has resized
	ResizeNvim { rows: u16, cols: u16 },
}

pub async fn run<B: ratatui::backend::Backend>(
	bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	terminal: Arc<RefCell<Terminal<B>>>,
) -> Result<!> {
	let mut debug_mouse_areas = false;

	let mouse_areas = RefCell::new(Vec::new());

	let main_split = ui::SplitState::new(0.8);
	let gdb_split = ui::SplitState::new(0.2);
	let code_split = ui::SplitState::new(0.75);
	let glance_split = ui::SplitState::new(0.2);
	let control_split = ui::SplitState::new(0.4);

	let mut video_online = false;
	let mut video_size = (640, 480);
	let mut video_state = ui::Video::default();

	let mut gdb_tui = ui::Tui::default();
	let mut qemu_tui = ui::Tui::default();
	let mut nvim_tui = ui::Tui::default();

	let mut focus = Focus::default();

	let mut debug_state = Arc::new(orok_test_harness::State::for_arch::<
		orok_test_harness::X8664State,
	>());

	loop {
		mouse_areas.borrow_mut().clear();

		let pass = ui::Pass {
			mouse_areas: &mouse_areas,
		};

		{
			terminal
				.borrow_mut()
				.draw(|f| {
					let area = f.area();
					let buf = f.buffer_mut();

					ui::Split::Horizontal(
						&pass,
						&main_split,
						ui::Split::Vertical(
							&pass,
							&gdb_split,
							ui::Split::Horizontal(
								&pass,
								&glance_split,
								ui::Titled(
									"QEMU (Alt-Q)",
									ui::Focused(
										focus,
										Focus::Qemu,
										qemu_tui.render(|ui::TuiSize { rows, cols }| {
											let bus = Arc::clone(&bus);
											tokio::spawn(async move {
												if let Err(err) = bus
													.tui
													.send(Event::ResizeQemu { rows, cols })
													.await
												{
													log::error!(
														"failed to send Resize event to qemu \
														 service: {err}"
													);
												}
											});
										}),
									),
								),
								ui::Titled(
									"GDB (Alt-G)",
									ui::Focused(
										focus,
										Focus::Gdb,
										gdb_tui.render(|ui::TuiSize { rows, cols }| {
											let bus = Arc::clone(&bus);
											tokio::spawn(async move {
												if let Err(err) = bus
													.tui
													.send(Event::ResizeGdb { rows, cols })
													.await
												{
													log::error!(
														"failed to send Resize event to gdb: {err}"
													);
												}
											});
										}),
									),
								),
							),
							ui::Split::Vertical(
								&pass,
								&code_split,
								ui::Titled(
									"Code View (Alt-C)",
									ui::Focused(
										focus,
										Focus::Nvim,
										nvim_tui.render(|ui::TuiSize { rows, cols }| {
											let bus = Arc::clone(&bus);
											tokio::spawn(async move {
												if let Err(err) = bus
													.tui
													.send(Event::ResizeNvim { rows, cols })
													.await
												{
													log::error!(
														"failed to send Resize event to nvim: \
														 {err}"
													);
												}
											});
										}),
									),
								),
								ui::Split::Horizontal(
									&pass,
									&control_split,
									ui::Swap(
										video_online,
										ui::Titled("QEMU Video", ui::Status("[VIDEO OFFLINE]")),
										ui::Titled(
											&format!(
												"QEMU Video ({}x{})",
												video_size.0, video_size.1
											),
											video_state.render(),
										),
									),
									ControlPanel(&pass, &debug_state),
								),
							),
						),
						ui::Logger(&pass),
					)
					.render(area, buf);

					if debug_mouse_areas {
						for mouse_area in mouse_areas.borrow().iter() {
							let rect = match mouse_area {
								ui::MouseArea::LeftClick { rect, .. } => rect,
								ui::MouseArea::LeftClickDrag { rect, .. } => rect,
							};
							for y in rect.y..rect.bottom() {
								for x in rect.x..rect.right() {
									if x >= area.width || y >= area.height {
										continue;
									}
									if let Some(c) = buf.cell_mut((x, y)) {
										c.set_bg(ratatui::style::Color::Red);
									}
								}
							}
						}
					}
				})
				.map_err(|e| anyhow!("failed to render TUI: {e}"))?;
		}

		let capacity = mouse_areas.borrow().len();
		bus.input
			.send({
				let send_areas =
					std::mem::replace(&mut *mouse_areas.borrow_mut(), Vec::with_capacity(capacity));

				crate::service::input::Event::SetMouseAreas { areas: send_areas }
			})
			.await
			.with_context(|| "failed to send Event::SetMouseAreas")?;

		#[expect(clippy::never_loop)]
		loop {
			let Some(evt) = rx.recv().await else {
				bail!("EOF");
			};

			match evt {
				Event::Invalidate => {
					break;
				}
				Event::ShowMouseAreas { show } => {
					debug_mouse_areas = show.unwrap_or(!debug_mouse_areas);
					break;
				}
				Event::VideoFrame { image } => {
					video_state.set_image(image);
					break;
				}
				Event::VideoDisconnected => {
					video_online = false;
					break;
				}
				Event::VideoConnected => {
					video_online = true;
					break;
				}
				Event::VideoSizeChanged { width, height } => {
					video_size = (width, height);
					break;
				}
				Event::GdbStarted => {
					gdb_tui.reset();
					break;
				}
				Event::GdbStdout { bytes } => {
					gdb_tui.parser.process(&bytes);
					break;
				}
				Event::FullInvalidate => {
					terminal
						.borrow_mut()
						.clear()
						.map_err(|e| anyhow!("failed to clear terminal: {e}"))?;
					break;
				}
				Event::QemuStarted => {
					qemu_tui.reset();
					break;
				}
				Event::QemuStdout { bytes } => {
					qemu_tui.parser.process(&bytes);
					break;
				}
				Event::SetFocus { focus: new_focus } => {
					focus = new_focus;
					log::trace!("new focus: {focus:?}");
					break;
				}
				Event::ResizeGdb { rows, cols } => {
					bus.gdb
						.send(crate::service::gdb::Event::Resize { rows, cols })
						.await
						.with_context(|| "failed to send Resize event to gdb service")?;
					gdb_tui.resize(rows, cols);
					break;
				}
				Event::ResizeQemu { rows, cols } => {
					bus.qemu
						.send(crate::service::qemu::Event::Resize { rows, cols })
						.await
						.with_context(|| "failed to send Resize event to qemu service")?;
					qemu_tui.resize(rows, cols);
					break;
				}
				Event::SetDebugState {
					debug_state: new_state,
				} => {
					debug_state = new_state;
					break;
				}
				Event::NvimStdout { bytes } => {
					nvim_tui.parser.process(&bytes);
					break;
				}
				Event::ResizeNvim { rows, cols } => {
					bus.nvim
						.send(crate::service::nvim::Event::Resize { rows, cols })
						.await
						.with_context(|| "failed to send Resize event to nvim service")?;
					nvim_tui.resize(rows, cols);
					break;
				}
			}
		}
	}
}

struct ControlPanel<'a>(&'a ui::Pass<'a>, &'a orok_test_harness::State);

impl<'a> Widget for ControlPanel<'a> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let [session_button_area, status_area, stats_area] = Layout::default()
			.direction(Direction::Vertical)
			.constraints([
				Constraint::Fill(1),
				Constraint::Fill(2),
				Constraint::Fill(2),
			])
			.spacing(Spacing::Space(1))
			.areas(area);

		let [scram_button, x86_64_button, aarch64_button] = Layout::default()
			.direction(Direction::Vertical)
			.constraints([Constraint::Min(3), Constraint::Min(1), Constraint::Min(1)])
			.spacing(Spacing::Space(1))
			.areas(session_button_area.crop_top(2).crop_bottom(1));

		ui::Titled("Session Control", "").render(session_button_area, buf);
		ui::Titled("Status", "").render(status_area, buf);
		ui::Titled("Debug State", ui::DebugState(self.1)).render(stats_area, buf);

		ui::Button::new(self.0, "SCRAM")
			.style(Style::new().bg(Color::Indexed(124)).fg(Color::White))
			.on_click({
				move |_, bus| {
					let bus = Arc::clone(&bus);
					tokio::spawn(async move {
						if let Err(err) = bus
							.session
							.send(crate::service::session::Event::Scram)
							.await
						{
							log::error!("failed to send scram event: {err}");
						}
					});
					false
				}
			})
			.render(scram_button, buf);

		ui::Button::new(self.0, "[new session] x86_64")
			.on_click({
				move |_, bus| {
					let bus = Arc::clone(&bus);
					tokio::spawn(async move {
						if let Err(err) = bus
							.session
							.send(crate::service::session::Event::StartSession {
								arch: crate::service::session::Arch::X86_64,
							})
							.await
						{
							log::error!("failed to send StartSession(x86_64) event: {err}");
						}
					});
					false
				}
			})
			.render(x86_64_button, buf);

		ui::Button::new(self.0, "[new session] AArch64")
			.on_click({
				move |_, bus| {
					let bus = Arc::clone(&bus);
					tokio::spawn(async move {
						if let Err(err) = bus
							.session
							.send(crate::service::session::Event::StartSession {
								arch: crate::service::session::Arch::Aarch64,
							})
							.await
						{
							log::error!("failed to send StartSession(AArch64) event: {err}");
						}
					});
					false
				}
			})
			.render(aarch64_button, buf);
	}
}

impl<'a> ui::BackgroundWidget for ControlPanel<'a> {}
