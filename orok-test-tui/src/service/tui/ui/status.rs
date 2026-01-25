use ratatui::{
	buffer::Buffer,
	layout::{Alignment, Rect},
	style::{Color, Style},
	widgets::{Paragraph, Widget},
};

use super::BackgroundWidget;

pub struct Status<'a>(pub &'a str);

impl Widget for Status<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		if area.height > 0 {
			let centered_area = Rect {
				x:      area.x,
				y:      area.y + area.height / 2,
				width:  area.width,
				height: 1,
			};
			Paragraph::new(self.0)
				.alignment(Alignment::Center)
				.style(Style::default().fg(Color::DarkGray))
				.render(centered_area, buf)
		}
	}
}

impl BackgroundWidget for Status<'_> {}
