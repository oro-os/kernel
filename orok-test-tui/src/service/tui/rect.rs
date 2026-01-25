use ratatui::layout::Rect;

#[allow(unused)]
pub trait RectExt: Sized {
	fn into_rect(self) -> Rect;

	fn crop_top(self, n: u16) -> Rect {
		let this = self.into_rect();
		Rect {
			x:      this.x,
			y:      this.y + n,
			width:  this.width,
			height: this.height.saturating_sub(n),
		}
	}

	fn crop_bottom(self, n: u16) -> Rect {
		let this = self.into_rect();
		Rect {
			x:      this.x,
			y:      this.y,
			width:  this.width,
			height: this.height.saturating_sub(n),
		}
	}

	fn crop_left(self, n: u16) -> Rect {
		let this = self.into_rect();
		Rect {
			x:      this.x + n,
			y:      this.y,
			width:  this.width.saturating_sub(n),
			height: this.height,
		}
	}

	fn crop_right(self, n: u16) -> Rect {
		let this = self.into_rect();
		Rect {
			x:      this.x,
			y:      this.y,
			width:  this.width.saturating_sub(n),
			height: this.height,
		}
	}

	fn padded(self, pad: impl IntoPadding) -> Rect {
		let this = self.into_rect();
		let (top, right, bottom, left) = pad.into_padding();
		Rect {
			x:      this.x + left,
			y:      this.y + top,
			width:  this.width.saturating_sub(right + left),
			height: this.height.saturating_sub(top + bottom),
		}
	}
}

impl RectExt for Rect {
	#[inline]
	fn into_rect(self) -> Rect {
		self
	}
}

pub trait IntoPadding {
	fn into_padding(self) -> (u16, u16, u16, u16);
}

impl IntoPadding for u16 {
	fn into_padding(self) -> (u16, u16, u16, u16) {
		(self, self, self, self)
	}
}

impl IntoPadding for (u16, u16) {
	fn into_padding(self) -> (u16, u16, u16, u16) {
		(self.0, self.1, self.0, self.1)
	}
}

impl IntoPadding for (u16, u16, u16, u16) {
	fn into_padding(self) -> (u16, u16, u16, u16) {
		self
	}
}
