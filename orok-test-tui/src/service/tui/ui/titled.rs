use ratatui::{
	buffer::Buffer,
	layout::Rect,
	style::{Color, Style},
	text::Span,
	widgets::Widget,
};

use super::BackgroundWidget;

pub struct Titled<'a, Content>(pub &'a str, pub Content);

const TITLE_STYLE: Style = Style::new().fg(Color::Gray).bg(Color::Indexed(235));

impl<'a, Content> Widget for Titled<'a, Content>
where
	Content: Widget + BackgroundWidget,
{
	fn render(self, area: Rect, buf: &mut Buffer) {
		if area.height > 0 {
			let title_area = Rect {
				width:  area.width,
				height: 1,
				x:      area.x,
				y:      area.y,
			};
			buf.set_style(title_area, TITLE_STYLE);

			Span::from(self.0)
				.style(TITLE_STYLE)
				.render(title_area, buf);
		}

		let content_area = Rect {
			x:      area.x,
			y:      area.y + 1,
			width:  area.width,
			height: area.height.saturating_sub(1),
		};

		if content_area.height > 0 {
			self.1.render(content_area, buf);
		} else {
			self.1.set_area(content_area);
		}
	}
}

impl<'a, Content> BackgroundWidget for Titled<'a, Content>
where
	Content: Widget + BackgroundWidget,
{
	fn set_area(self, area: Rect)
	where
		Self: Sized,
	{
		self.1.set_area(area);
	}
}
