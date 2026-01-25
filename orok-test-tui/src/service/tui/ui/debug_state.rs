use orok_test_harness::State;
use ratatui::{
	buffer::Buffer,
	layout::{Constraint, Rect},
	prelude::Stylize,
	style::Color,
	text::{Line, Span},
	widgets::{Row, Table, Widget},
};

use super::BackgroundWidget;
use crate::{atomic::RelaxedAtomic, service::tui::rect::RectExt};
struct Field(
	&'static str,
	usize,
	&'static (dyn Fn(&State) -> Span + Sync + Send + 'static),
);

macro_rules! true_ok {
	($name:literal, $f:ident) => {
		Field($name, 6, &|s| {
			if s.$f.get() {
				Span::from("ok").fg(Color::LightGreen)
			} else {
				Span::from("not ok").fg(Color::LightRed)
			}
		})
	};
}

static FIELDS: &[Field] = &[
	true_ok!("initialized?", initialized),
	Field("total packets", 16, &|s| {
		Span::from(s.packet_count.get().to_string())
	}),
	Field("events", 16, &|s| {
		let count = s.event_count.get();
		let s = Span::from(count.to_string());
		if count > 0 {
			s.fg(Color::LightYellow)
		} else {
			s
		}
	}),
	Field("skipped checks", 16, &|s| {
		Span::from(s.skipped_constraints.get().to_string())
	}),
	true_ok!("in kernel?", in_kernel),
	Field("last dbgloc", 16, &|s| {
		Span::from(s.last_debug_loc_offset.get().to_string())
	}),
];

pub struct DebugState<'a>(pub &'a State);

impl Widget for DebugState<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut max_name_width = 0;
		let mut max_value_width = 0;

		for field in FIELDS {
			max_name_width = field.0.len().max(max_name_width);
			max_value_width = field.1.max(max_value_width);
		}

		let rows = FIELDS
			.iter()
			.map(|f| Row::new(vec![Line::from(f.0), f.2(self.0).into_right_aligned_line()]))
			.collect::<Vec<_>>();

		Table::new(
			rows,
			[
				Constraint::Min(max_name_width as u16),
				Constraint::Min(max_value_width as u16),
			],
		)
		.column_spacing(1)
		.render(area.padded((0, 1)), buf);
	}
}

impl BackgroundWidget for DebugState<'_> {}
