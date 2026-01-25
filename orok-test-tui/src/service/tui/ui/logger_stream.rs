use ratatui::{
	buffer::Buffer,
	layout::Rect,
	style::{Color, Style},
	text::{Line, Span},
	widgets::Widget,
};

use crate::logging::LogRecord;

const MIN_WIDTH: usize = 40;

/// A widget that combines the logger with an overlay in the top-right corner
pub struct LoggerStream;

impl Widget for LoggerStream {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut i = 0;
		crate::logging::for_each_log_reverse(|message| {
			if i >= area.height as usize {
				return false;
			}
			if let Some(line) = format(area.width as usize, message) {
				let y = area.y + area.height.saturating_sub(1).saturating_sub(i as u16);
				buf.set_line(area.x, y, &line, area.width);
				i += 1;
			}
			true
		});
	}
}

fn format(width: usize, evt: &LogRecord) -> Option<Line<'_>> {
	let level_str = match evt.level {
		log::Level::Error => "[ERROR] ",
		log::Level::Warn => "[WARN ] ",
		log::Level::Info => "[INFO ] ",
		log::Level::Debug => "[DEBUG] ",
		log::Level::Trace => "[TRACE] ",
	};
	let prefix_len = level_str.len() + evt.target.len() + 2; // +2 for ": "

	let _available_width = width
		.saturating_sub(prefix_len)
		.max(MIN_WIDTH.saturating_sub(prefix_len + 1));

	Some(build_log_line(
		evt.level,
		level_str,
		&evt.target,
		&evt.message,
		true,
	))
}

fn build_log_line(
	level: log::Level,
	level_str: &str,
	target: &str,
	msg: &str,
	_wrapped: bool,
) -> Line<'static> {
	match level {
		log::Level::Error => {
			// Red for errors
			Line::from(vec![
				Span::styled(level_str.to_string(), Style::default().fg(Color::Red)),
				Span::styled(
					format!("{}: ", target),
					Style::default().fg(Color::DarkGray),
				),
				Span::raw(msg.to_string()),
			])
		}
		log::Level::Warn => {
			// Yellow for warnings
			Line::from(vec![
				Span::styled(level_str.to_string(), Style::default().fg(Color::Yellow)),
				Span::styled(
					format!("{}: ", target),
					Style::default().fg(Color::DarkGray),
				),
				Span::raw(msg.to_string()),
			])
		}
		log::Level::Info => {
			// Green for info
			Line::from(vec![
				Span::styled(level_str.to_string(), Style::default().fg(Color::Green)),
				Span::styled(
					format!("{}: ", target),
					Style::default().fg(Color::DarkGray),
				),
				Span::raw(msg.to_string()),
			])
		}
		log::Level::Debug => {
			// Dark purple for debug (ANSI 256 color 55, except module)
			Line::from(vec![
				Span::styled(
					level_str.to_string(),
					Style::default().fg(Color::Indexed(55)),
				),
				Span::styled(
					format!("{}: ", target),
					Style::default().fg(Color::DarkGray),
				),
				Span::styled(msg.to_string(), Style::default().fg(Color::Indexed(55))),
			])
		}
		log::Level::Trace => {
			// Dim/gray for entire trace line
			Line::from(vec![
				Span::styled(level_str.to_string(), Style::default().fg(Color::DarkGray)),
				Span::styled(
					format!("{}: ", target),
					Style::default().fg(Color::DarkGray),
				),
				Span::styled(msg.to_string(), Style::default().fg(Color::DarkGray)),
			])
		}
	}
}
