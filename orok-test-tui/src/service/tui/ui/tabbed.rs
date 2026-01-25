use std::{cell::Cell, sync::Arc};

use crossterm::event::MouseEvent;
use ratatui::{
	buffer::Buffer,
	layout::Rect,
	style::{Color, Modifier, Style},
	text::Span,
	widgets::Widget,
};

use crate::ui::{MouseArea, Pass, widgets::BackgroundWidget};

const SELECTED_STYLE: Style = Style::new()
	.fg(Color::White)
	.bg(Color::Indexed(90))
	.add_modifier(Modifier::BOLD);
const UNSELECTED_STYLE: Style = Style::new().fg(Color::Gray).bg(Color::DarkGray);

pub struct Tabbed<'a> {
	pass:     &'a Pass<'a>,
	tabs:     Vec<(&'a str, Box<dyn LightWidgetHandle + 'a>)>,
	selected: usize,
	handler:  Option<Arc<dyn Fn(usize) + Send + Sync + 'static>>,
}

impl<'a> Tabbed<'a> {
	pub fn new(pass: &'a Pass<'a>, selected: usize) -> Self {
		Self {
			pass,
			tabs: Vec::new(),
			selected,
			handler: None,
		}
	}

	pub fn tab(mut self, title: &'a str, content: impl BackgroundWidget + 'a) -> Self {
		self.tabs.push((title, Box::new(LightWidget::new(content))));
		self
	}

	pub fn on_tab_change<F: Fn(usize) + Send + Sync + 'static>(mut self, f: F) -> Self {
		self.handler = Some(Arc::new(f));
		self
	}
}

impl<'a> Widget for Tabbed<'a> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let content_area = Rect {
			x:      area.x,
			y:      area.y + 1,
			width:  area.width,
			height: area.height.saturating_sub(1),
		};

		let mut offset = usize::from(area.x);

		for (i, (title, mut content)) in self.tabs.into_iter().enumerate() {
			let style = if i == self.selected {
				content.try_render(content_area, buf);
				SELECTED_STYLE
			} else {
				content.try_set_size(content_area);
				UNSELECTED_STYLE
			};

			let title = Span::from(title).style(style);
			let width = title.width().saturating_sub(
				usize::from(area.x)
					.saturating_add(offset)
					.saturating_sub(usize::from(area.width)),
			);

			if width > 0 {
				let title_area = Rect {
					x:      offset as u16,
					y:      area.y,
					width:  width as u16,
					height: 1,
				};

				if let Some(handler) = self.handler.clone() {
					self.pass.push_mouse_area(MouseArea::LeftClick {
						rect:    title_area,
						handler: Box::from(
							move |_ev: &MouseEvent, _state: &mut crate::ui::State| {
								handler(i);
								true
							},
						),
					});
				}

				title.render(title_area, buf);

				offset += width + 1;
			}
		}
	}
}

struct LightWidget<T> {
	inner: Cell<Option<T>>,
}

impl<T> LightWidget<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner: Cell::new(Some(inner)),
		}
	}
}

trait LightWidgetHandle {
	fn try_render(&mut self, area: Rect, buf: &mut Buffer);
	fn try_set_size(&mut self, area: Rect);
}

impl<T> LightWidgetHandle for LightWidget<T>
where
	T: BackgroundWidget,
{
	fn try_render(&mut self, area: Rect, buf: &mut Buffer) {
		if let Some(inner) = self.inner.take() {
			inner.render(area, buf);
		}
	}

	fn try_set_size(&mut self, area: Rect) {
		if let Some(inner) = self.inner.take() {
			inner.set_area(area);
		}
	}
}
