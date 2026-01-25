use std::sync::atomic::{AtomicU16, Ordering::Relaxed};

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tui_term::widget::PseudoTerminal;

use super::BackgroundWidget;

#[derive(Default)]
pub struct Tui {
	pub parser: vt100::Parser,
	last_size:  (AtomicU16, AtomicU16),
}

impl Tui {
	pub fn render<F>(&self, on_resize: F) -> TuiRenderer<'_, F>
	where
		F: FnOnce(TuiSize),
	{
		TuiRenderer {
			widget: self,
			on_resize,
		}
	}

	pub fn resize(&mut self, rows: u16, cols: u16) {
		self.last_size.0.store(cols, Relaxed);
		self.last_size.1.store(rows, Relaxed);
		self.parser.screen_mut().set_size(rows, cols);
	}

	pub fn reset(&mut self) {
		let (w, h) = (
			self.last_size.0.load(Relaxed),
			self.last_size.1.load(Relaxed),
		);
		self.parser = vt100::Parser::new(h, w, 0);
	}
}

pub struct TuiRenderer<'a, F> {
	widget:    &'a Tui,
	on_resize: F,
}

pub struct TuiSize {
	pub cols: u16,
	pub rows: u16,
}

impl<F> TuiRenderer<'_, F>
where
	F: FnOnce(TuiSize),
{
	fn update_area(self, area: Rect) {
		let (new_width, new_height) = (area.width, area.height);
		let last_width = self.widget.last_size.0.swap(new_width, Relaxed);
		let last_height = self.widget.last_size.1.swap(new_height, Relaxed);
		if last_width != new_width || last_height != new_height {
			(self.on_resize)(TuiSize {
				cols: new_width,
				rows: new_height,
			});
		}
	}
}

impl<F> Widget for TuiRenderer<'_, F>
where
	F: FnOnce(TuiSize),
{
	fn render(self, area: Rect, buf: &mut Buffer) {
		PseudoTerminal::new(self.widget.parser.screen()).render(area, buf);
		self.update_area(area);
	}
}

impl<F> BackgroundWidget for TuiRenderer<'_, F>
where
	F: FnOnce(TuiSize),
{
	fn set_area(self, area: Rect) {
		self.update_area(area);
	}
}
