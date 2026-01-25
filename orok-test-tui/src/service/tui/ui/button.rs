use std::sync::Arc;

use crossterm::event::MouseEvent;
use ratatui::{
	buffer::Buffer,
	layout::{Alignment, Rect},
	style::{Color, Modifier, Style},
	text::Line,
	widgets::Widget,
};

use super::{BackgroundWidget, MouseArea, Pass};

const DEFAULT_STYLE: Style = Style::new()
	.bg(Color::Indexed(254))
	.fg(Color::Black)
	.add_modifier(Modifier::BOLD);

pub struct Button<'a> {
	pass:     &'a Pass<'a>,
	text:     &'a str,
	#[expect(clippy::type_complexity)]
	callback: Option<Arc<dyn Fn(&MouseEvent, Arc<crate::Bus>) -> bool + Send + Sync + 'static>>,
	style:    Style,
}

impl<'a> Button<'a> {
	pub fn new(pass: &'a Pass<'a>, text: &'a str) -> Self {
		Self {
			pass,
			text,
			callback: None,
			style: DEFAULT_STYLE,
		}
	}

	pub fn on_click<F: Fn(&MouseEvent, Arc<crate::Bus>) -> bool + Send + Sync + 'static>(
		mut self,
		f: F,
	) -> Self {
		self.callback = Some(Arc::new(f));
		self
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = DEFAULT_STYLE.patch(style);
		self
	}
}

impl Widget for Button<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		if let Some(callback) = self.callback {
			self.pass.push_mouse_area(MouseArea::LeftClick {
				rect:    area,
				handler: callback,
			});
		}

		buf.set_style(area, self.style);

		let span_area = Rect {
			x:      area.x,
			y:      area.y + area.height / 2,
			width:  area.width,
			height: 1,
		};

		Line::from(self.text)
			.style(self.style)
			.alignment(Alignment::Center)
			.render(span_area, buf);
	}
}

impl BackgroundWidget for Button<'_> {}
