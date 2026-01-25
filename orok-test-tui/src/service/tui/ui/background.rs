use ratatui::{layout::Rect, widgets::Widget};

pub trait BackgroundWidget: Widget {
	fn set_area(self, _area: Rect)
	where
		Self: Sized,
	{
	}
}

impl BackgroundWidget for &str {}
impl BackgroundWidget for String {}

impl<W: BackgroundWidget> BackgroundWidget for Option<W> {
	fn set_area(self, area: Rect)
	where
		Self: Sized,
	{
		if let Some(inner) = self {
			inner.set_area(area);
		}
	}
}

impl BackgroundWidget for ratatui::widgets::BarChart<'_> {}
impl BackgroundWidget for ratatui::widgets::Block<'_> {}
impl BackgroundWidget for ratatui::widgets::Chart<'_> {}
impl BackgroundWidget for ratatui::widgets::Clear {}
impl BackgroundWidget for ratatui::widgets::Gauge<'_> {}
impl BackgroundWidget for ratatui::widgets::LineGauge<'_> {}
impl BackgroundWidget for ratatui::widgets::List<'_> {}
impl BackgroundWidget for ratatui::widgets::Paragraph<'_> {}
impl BackgroundWidget for ratatui::widgets::RatatuiLogo {}
impl BackgroundWidget for ratatui::widgets::RatatuiMascot {}
impl BackgroundWidget for ratatui::widgets::Sparkline<'_> {}
impl BackgroundWidget for ratatui::widgets::Table<'_> {}
