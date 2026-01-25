use std::sync::Arc;

use ratatui::{
	buffer::Buffer,
	layout::Rect,
	style::{Color, Style},
	text::Span,
	widgets::Widget,
};

use super::{MouseArea, Pass};
pub struct LoggerFilter<'a>(pub &'a Pass<'a>);

impl<'a> Widget for LoggerFilter<'a> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		const LEVEL_CHARS: &str = "EWIDT";

		if area.width == 0 || area.height == 0 {
			return;
		}

		let mut current_x = area.x;

		// Clear the left padding area (one character)
		// Retains existing background/style
		if current_x < area.right()
			&& let Some(c) = buf.cell_mut((area.x, area.y))
		{
			c.set_symbol(" ");
		}

		if (area.y + 1) < area.bottom()
			&& let Some(c) = buf.cell_mut((area.x, area.y + 1))
		{
			c.set_symbol(" ");
		}

		current_x += 1;

		for (i, ch) in LEVEL_CHARS.chars().enumerate() {
			let level = match i {
				0 => log::Level::Error,
				1 => log::Level::Warn,
				2 => log::Level::Info,
				3 => log::Level::Debug,
				4 => log::Level::Trace,
				_ => unreachable!(),
			};

			let style = if crate::logging::is_enabled(level) {
				// Inverted: black text on white background
				Style::default().fg(Color::Black).bg(Color::White)
			} else {
				// Normal: default colors
				Style::default()
			};

			if current_x < area.right() {
				buf.set_span(current_x, area.y, &Span::styled(ch.to_string(), style), 1);

				let toggle_level = match i {
					0 => log::Level::Error,
					1 => log::Level::Warn,
					2 => log::Level::Info,
					3 => log::Level::Debug,
					4 => log::Level::Trace,
					_ => unreachable!(),
				};

				self.0.push_mouse_area(MouseArea::LeftClick {
					rect:    Rect {
						x:      current_x,
						y:      area.y,
						width:  1,
						height: 1,
					},
					handler: Arc::new(move |_event, _bus| {
						crate::logging::toggle_level(toggle_level);
						true
					}),
				});
			} else {
				break;
			}

			current_x += 1;
		}
	}
}
