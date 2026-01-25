use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

use super::BackgroundWidget;
use crate::service::input::Focus;

const FOCUSED_BG: Color = Color::Indexed(232);

pub struct Focused<T: BackgroundWidget>(pub Focus, pub Focus, pub T);

impl<T> Widget for Focused<T>
where
	T: BackgroundWidget,
{
	fn render(self, area: Rect, buf: &mut Buffer) {
		self.2.render(area, buf);

		// This is used on TUIs for the most part,
		// and they force-render the entire thing back to RESET.
		// So we have to force them back to non-RESET. Manually.
		if self.0 == self.1 {
			for x in area.left()..area.right() {
				for y in area.top()..area.bottom() {
					let Some(cell) = buf.cell_mut((x, y)) else {
						continue;
					};

					if cell.bg == Color::Reset {
						cell.bg = FOCUSED_BG;
					}
				}
			}
		}
	}
}

impl<T> BackgroundWidget for Focused<T>
where
	T: BackgroundWidget,
{
	fn set_area(self, area: Rect)
	where
		Self: Sized,
	{
		self.2.set_area(area);
	}
}
