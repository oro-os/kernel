mod background;
mod button;
mod debug_state;
mod focused;
mod logger;
mod logger_filter;
mod logger_stream;
mod split;
mod status;
mod swap;
mod titled;
mod tui;
mod video;

use std::{cell::RefCell, sync::Arc};

use crossterm::event::MouseEvent;

pub use self::{
	background::BackgroundWidget,
	button::Button,
	debug_state::DebugState,
	focused::Focused,
	logger::Logger,
	logger_filter::LoggerFilter,
	logger_stream::LoggerStream,
	split::{Split, SplitState},
	status::Status,
	swap::Swap,
	titled::Titled,
	tui::{Tui, TuiSize},
	video::Video,
};

pub struct Pass<'a> {
	pub(super) mouse_areas: &'a RefCell<Vec<MouseArea>>,
}

impl<'a> Pass<'a> {
	pub fn push_mouse_area(&self, area: MouseArea) {
		self.mouse_areas.borrow_mut().push(area);
	}
}

pub enum MouseArea {
	LeftClick {
		rect:    ratatui::layout::Rect,
		#[expect(clippy::type_complexity)]
		handler: Arc<dyn Fn(&MouseEvent, Arc<crate::Bus>) -> bool + Send + Sync>,
	},
	LeftClickDrag {
		rect:    ratatui::layout::Rect,
		#[expect(clippy::type_complexity)]
		handler: Arc<dyn Fn(&MouseEvent, Arc<crate::Bus>) -> bool + Send + Sync>,
	},
}
