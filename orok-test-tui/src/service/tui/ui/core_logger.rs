use ratatui::{
	buffer::Buffer,
	layout::Rect,
	prelude::Stylize,
	style::Color,
	text::{Line, Span},
	widgets::Widget,
};

use super::BackgroundWidget;

// Taken from `debug-js/debug`.
const CORE_COLORS: &[u8] = &[
	20, 21, 26, 27, 32, 33, 38, 39, 40, 41, 42, 43, 44, 45, 56, 57, 62, 63, 68, 69, 74, 75, 76, 77,
	78, 79, 80, 81, 92, 93, 98, 99, 112, 113, 128, 129, 134, 135, 148, 149, 160, 161, 162, 163,
	164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 178, 179, 184, 185, 196, 197, 198, 199, 200,
	201, 202, 203, 204, 205, 206, 207, 208, 209, 214, 215, 220, 221,
];

pub struct CoreLogMessage {
	pub core_id: usize,
	pub level:   orok_test_harness::LogLevel,
	pub message: String,
}

pub struct CoreLogger<'a>(pub &'a [CoreLogMessage]);

impl Widget for CoreLogger<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut y = area.bottom() - 1;
		for CoreLogMessage {
			core_id,
			level,
			message,
		} in self.0.iter().rev()
		{
			if y < area.top() {
				break;
			}

			let color = CORE_COLORS[core_id % CORE_COLORS.len()];
			let core_span = Span::from(format!("[{core_id}]")).fg(Color::Indexed(color));
			let level_span = Span::from(format!("{level:?}")).fg(match level {
				orok_test_harness::LogLevel::Trace => Color::DarkGray,
				orok_test_harness::LogLevel::Debug => Color::Blue,
				orok_test_harness::LogLevel::Info => Color::Green,
				orok_test_harness::LogLevel::Warn => Color::Yellow,
				orok_test_harness::LogLevel::Error => Color::Red,
			});
			let message_span = Span::from(message.as_str()).fg(Color::White);

			let line = Line::from(vec![
				core_span,
				Span::from(" "),
				level_span,
				Span::from(" "),
				message_span,
			]);

			buf.set_line(area.left(), y, &line, area.width);
			y -= 1;
		}
	}
}

impl BackgroundWidget for CoreLogger<'_> {}
