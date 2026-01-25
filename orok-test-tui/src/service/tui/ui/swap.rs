use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::BackgroundWidget;

pub struct Swap<True, False>(pub bool /* swap? */, pub False, pub True);

impl<True, False> Widget for Swap<True, False>
where
	True: Widget + BackgroundWidget,
	False: Widget + BackgroundWidget,
{
	fn render(self, area: Rect, buf: &mut Buffer) {
		if self.0 {
			self.1.set_area(area);
			self.2.render(area, buf);
		} else {
			self.2.set_area(area);
			self.1.render(area, buf);
		}
	}
}

impl<True, False> BackgroundWidget for Swap<True, False>
where
	True: Widget + BackgroundWidget,
	False: Widget + BackgroundWidget,
{
	fn set_area(self, area: Rect)
	where
		Self: Sized,
	{
		self.1.set_area(area);
		self.2.set_area(area);
	}
}
