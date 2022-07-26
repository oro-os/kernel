use core::cmp::min;

pub enum PixelFormat {
	RGB8,
	BGR8,
	GREY8,
	FALLBACK,
}

pub struct RasterizerInfo {
	pub format: PixelFormat,
	pub width: usize,
	pub height: usize,
	pub stride: usize,
	pub pixel_stride: usize,
}

pub struct Rasterizer<'a> {
	info: RasterizerInfo,
	color: [u8; 8],
	pixel_size: usize,
	buffer: &'a mut [u8],
}

impl<'a> Rasterizer<'a> {
	pub fn new(buffer: &'a mut [u8], info: RasterizerInfo) -> Self {
		let pixel_size = match info.format {
			PixelFormat::RGB8 => 3,
			PixelFormat::BGR8 => 3,
			PixelFormat::GREY8 => 1,
			PixelFormat::FALLBACK => min(info.pixel_stride, 8),
		};

		Self {
			info: info,
			color: [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
			buffer: buffer,
			pixel_size: pixel_size,
		}
	}

	pub fn set_accent_color(self: &mut Self) {
		// A pre-determined accent color for the Oro VGA displays.
		self.set_color(0x4D, 0x00, 0xE7, 0x40);
	}

	pub fn set_color(self: &mut Self, r: u8, g: u8, b: u8, grey: u8) {
		match self.info.format {
			PixelFormat::RGB8 => {
				self.color[0] = r;
				self.color[1] = g;
				self.color[2] = b;
			}
			PixelFormat::BGR8 => {
				self.color[0] = b;
				self.color[1] = g;
				self.color[2] = r;
			}
			PixelFormat::GREY8 => {
				self.color[0] = grey;
				self.color[1] = grey;
				self.color[2] = grey;
			}
			PixelFormat::FALLBACK => {
				for i in 0..self.pixel_size {
					self.color[i] = grey;
				}
			}
		};
	}

	pub fn clear(self: &mut Self) {
		for y in 0..self.info.height {
			for x in 0..self.info.width {
				self.mark_unsafe(x, y);
			}
		}
	}

	pub fn draw_frame(self: &mut Self) {
		let right = self.info.width - 5;
		let bottom = self.info.height - 5;

		for x in 5..right {
			self.mark_unsafe(x, 5);
			self.mark_unsafe(x, bottom);
		}
		for y in 5..bottom {
			self.mark_unsafe(5, y);
			self.mark_unsafe(right, y);
		}
		self.mark_unsafe(right, bottom);

		let br = right - 90;
		for y in 5..20 {
			for x in br..right {
				self.mark_unsafe(x, y);
			}
		}

		for by in 0..15 {
			for bx in (br - (15 - by))..br {
				self.mark_unsafe(bx, by + 5);
			}
		}
	}

	pub fn mark(self: &mut Self, x: usize, y: usize) {
		if x >= self.info.width || y >= self.info.height {
			return;
		}
		self.mark_unsafe(x, y);
	}

	pub fn mark_unsafe(self: &mut Self, x: usize, y: usize) {
		let offset = (y * self.info.stride + x) * self.info.pixel_stride;
		for i in 0..self.pixel_size {
			self.buffer[offset + i] = self.color[i];
		}
	}
}
