use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::{LoggerFilter, LoggerStream, Pass};

pub struct Logger<'a>(pub &'a Pass<'a>);

impl<'a> Widget for Logger<'a> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		LoggerStream.render(area, buf);

		LoggerFilter(self.0).render(
			Rect {
				x:      area.x + area.width.saturating_sub(7),
				y:      area.y,
				width:  area.width.min(7),
				height: area.height.min(2),
			},
			buf,
		);
	}
}

impl super::BackgroundWidget for Logger<'_> {}
