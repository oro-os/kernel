use std::cell::RefCell;

use image::DynamicImage;
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui_image::{Resize, StatefulImage, picker::Picker, protocol::StatefulProtocol};

use super::BackgroundWidget;

pub struct Video {
	picker:      Picker,
	image_state: Option<RefCell<StatefulProtocol>>,
}

impl Default for Video {
	fn default() -> Self {
		let picker =
			Picker::from_query_stdio().expect("failed to initialize video protocol picker");

		Self {
			picker,
			image_state: None,
		}
	}
}

impl Video {
	pub fn set_image(&mut self, img: DynamicImage) {
		self.image_state = Some(RefCell::new(self.picker.new_resize_protocol(img)));
	}

	pub fn render(&self) -> impl BackgroundWidget + '_ {
		VideoRender {
			state: self.image_state.as_ref(),
		}
	}
}

struct VideoRender<'a> {
	state: Option<&'a RefCell<StatefulProtocol>>,
}

impl<'a> Widget for VideoRender<'a> {
	fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
		if let Some(state) = self.state {
			let image = StatefulImage::default().resize(Resize::Fit(None));
			image.render(area, buf, &mut *state.borrow_mut());
		}
	}
}

impl BackgroundWidget for VideoRender<'_> {}
