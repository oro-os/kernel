use std::sync::{
	Arc,
	atomic::{AtomicU16, Ordering::Relaxed},
};

use crossterm::event::MouseEvent;
use ratatui::{
	buffer::Buffer,
	layout::Rect,
	style::{Color, Style},
	widgets::Widget,
};

use super::{BackgroundWidget, MouseArea, Pass};

const SLIDER_STYLE: Style = Style::new().bg(Color::Indexed(234)).fg(Color::Indexed(236));

pub enum Split<'a, LT: BackgroundWidget + 'a, RB: BackgroundWidget + 'a> {
	Vertical(&'a Pass<'a>, &'a SplitState, LT, RB),
	Horizontal(&'a Pass<'a>, &'a SplitState, LT, RB),
}

pub struct SplitState(Arc<AtomicU16>);

impl SplitState {
	pub fn new(init_pct: f64) -> Self {
		SplitState(Arc::new(AtomicU16::new(
			(u16::MAX as f64 * init_pct) as u16,
		)))
	}
}

impl<'a, LT: BackgroundWidget + 'a, RB: BackgroundWidget + 'a> Widget for Split<'a, LT, RB> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		match self {
			Split::Vertical(pass, pos, l, r) => {
				let split_pos = pos.0.load(std::sync::atomic::Ordering::Relaxed) as f64;
				let split_pos = split_pos / (u16::MAX as f64) * (area.width as f64);
				let split_pos = split_pos.round() as u16;

				let left_arrow_pos = area.y + area.height / 2;
				let right_arrow_pos = left_arrow_pos + 1;

				let l_area = Rect {
					x:      area.x,
					y:      area.y,
					width:  split_pos,
					height: area.height,
				};
				if l_area.width > 0 && l_area.height > 0 {
					l.render(l_area, buf);
				} else {
					l.set_area(l_area);
				}

				let r_area = Rect {
					x:      area.x + split_pos + 1,
					y:      area.y,
					width:  area.width.saturating_sub(split_pos).saturating_sub(1),
					height: area.height,
				};
				if r_area.width > 0 && r_area.height > 0 {
					r.render(r_area, buf);
				} else {
					r.set_area(r_area);
				}

				for y in area.y..area.bottom() {
					if let Some(c) = buf.cell_mut((area.x + split_pos, y)) {
						match y {
							y if y == left_arrow_pos => {
								c.set_symbol("▶");
							}
							y if y == right_arrow_pos => {
								c.set_symbol("◀");
							}
							_ => {
								c.set_symbol(" ");
							}
						}
						c.set_style(SLIDER_STYLE);
					}
				}

				let split_pos_arc = Arc::clone(&pos.0);

				pass.push_mouse_area(MouseArea::LeftClickDrag {
					rect:    Rect {
						x:      area.x + split_pos,
						y:      area.y,
						width:  1,
						height: area.height,
					},
					handler: Arc::new(move |ev: &MouseEvent, _bus| {
						let new_split_pos = ev
							.column
							.saturating_sub(area.x)
							.max(area.x + 1)
							.min((area.x + area.width).saturating_sub(1));
						let new_split_pos =
							(new_split_pos as f64 / area.width as f64 * u16::MAX as f64) as u16;

						let last = split_pos_arc.swap(new_split_pos, Relaxed);
						last != ev.column
					}),
				});
			}
			Split::Horizontal(pass, pos, l, r) => {
				let split_pos = pos.0.load(std::sync::atomic::Ordering::Relaxed) as f64;
				let split_pos = (split_pos / (u16::MAX as f64)) * (area.height as f64);
				let split_pos = split_pos.round() as u16;

				let up_arrow_pos = area.x + area.width / 2;
				let down_arrow_pos = up_arrow_pos + 1;

				let l_area = Rect {
					x:      area.x,
					y:      area.y,
					width:  area.width,
					height: split_pos,
				};
				if l_area.width > 0 && l_area.height > 0 {
					l.render(l_area, buf);
				} else {
					l.set_area(l_area);
				}

				let r_area = Rect {
					x:      area.x,
					y:      area.y + split_pos + 1,
					width:  area.width,
					height: area.height.saturating_sub(split_pos).saturating_sub(1),
				};
				if r_area.height > 0 && r_area.width > 0 {
					r.render(r_area, buf);
				} else {
					r.set_area(r_area);
				}

				for x in area.x..area.right() {
					if let Some(c) = buf.cell_mut((x, area.y + split_pos)) {
						match x {
							x if x == up_arrow_pos => {
								c.set_symbol("▼");
							}
							x if x == down_arrow_pos => {
								c.set_symbol("▲");
							}
							_ => {
								c.set_symbol(" ");
							}
						}
						c.set_style(SLIDER_STYLE);
					}
				}

				let split_pos_arc = Arc::clone(&pos.0);

				pass.push_mouse_area(MouseArea::LeftClickDrag {
					rect:    Rect {
						x:      area.x,
						y:      area.y + split_pos,
						width:  area.width,
						height: 1,
					},
					handler: Arc::from(move |ev: &MouseEvent, _bus| {
						let new_split_pos = ev
							.row
							.saturating_sub(area.y)
							.max(area.y + 1)
							.min((area.y + area.height).saturating_sub(1));
						let new_split_pos =
							(new_split_pos as f64 / area.height as f64 * u16::MAX as f64) as u16;

						let last = split_pos_arc.swap(new_split_pos, Relaxed);
						last != ev.row
					}),
				});
			}
		}
	}
}

impl<L, R> BackgroundWidget for Split<'_, L, R>
where
	L: BackgroundWidget,
	R: BackgroundWidget,
{
	fn set_area(self, area: Rect) {
		match self {
			Split::Vertical(_, _, l, r) => {
				l.set_area(Rect {
					x:      area.x,
					y:      area.y,
					width:  area.width / 2,
					height: area.height,
				});
				r.set_area(Rect {
					x:      area.x + area.width / 2,
					y:      area.y,
					width:  area.width.saturating_sub(area.width / 2),
					height: area.height,
				});
			}
			Split::Horizontal(_, _, l, r) => {
				l.set_area(Rect {
					x:      area.x,
					y:      area.y,
					width:  area.width,
					height: area.height / 2,
				});
				r.set_area(Rect {
					x:      area.x,
					y:      area.y + area.height / 2,
					width:  area.width,
					height: area.height.saturating_sub(area.height / 2),
				});
			}
		}
	}
}
