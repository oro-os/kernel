use core::cmp::min;
use micromath::F32Ext;

// Kubasta Font by Kai Kubasta
// https://kai.kubasta.net/
const FONT_BITS: &'static [u8] = core::include_bytes!("font.bin");
const FONT_GLYPH_WIDTH: usize = 6;
const FONT_GLYPH_HEIGHT: usize = 13;
const FONT_GLYPH_STRIDE_BITS: usize = 552;

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

	pub fn mark_circle_outline(self: &mut Self, cx: usize, cy: usize, r: usize) {
		let mut d = (5 - (r as isize) * 4) / 4;
		let mut x = 0 as isize;
		let mut y = r as isize;

		let cxi = cx as isize;
		let cyi = cy as isize;

		loop {
			self.mark((cxi + x) as usize, (cyi + y) as usize);
			self.mark((cxi + x) as usize, (cyi - y) as usize);
			self.mark((cxi - x) as usize, (cyi + y) as usize);
			self.mark((cxi - x) as usize, (cyi - y) as usize);
			self.mark((cxi + y) as usize, (cyi + x) as usize);
			self.mark((cxi + y) as usize, (cyi - x) as usize);
			self.mark((cxi - y) as usize, (cyi + x) as usize);
			self.mark((cxi - y) as usize, (cyi - x) as usize);

			if d < 0 {
				d += 2 * x + 1;
			} else {
				d += 2 * (x - y) + 1;
				y -= 1;
			}

			x += 1;

			if x > y {
				break;
			};
		}
	}

	fn mark_line_to_y(self: &mut Self, x: usize, y: usize, to_y: usize) {
		if y < to_y {
			for py in y..to_y {
				self.mark(x, py);
			}
		} else {
			for py in to_y..y {
				self.mark(x, py);
			}
		}
	}

	pub fn mark_circle_fill(self: &mut Self, cx: usize, cy: usize, r: usize) {
		let mut d = (5 - (r as isize) * 4) / 4;
		let mut x = 0 as isize;
		let mut y = r as isize;

		let cxi = cx as isize;
		let cyi = cy as isize;

		loop {
			self.mark_line_to_y((cxi + x) as usize, (cyi + y) as usize, cy);
			self.mark_line_to_y((cxi + x) as usize, (cyi - y) as usize, cy);
			self.mark_line_to_y((cxi - x) as usize, (cyi + y) as usize, cy);
			self.mark_line_to_y((cxi - x) as usize, (cyi - y) as usize, cy);
			self.mark_line_to_y((cxi + y) as usize, (cyi + x) as usize, cy);
			self.mark_line_to_y((cxi + y) as usize, (cyi - x) as usize, cy);
			self.mark_line_to_y((cxi - y) as usize, (cyi + x) as usize, cy);
			self.mark_line_to_y((cxi - y) as usize, (cyi - x) as usize, cy);

			if d < 0 {
				d += 2 * x + 1;
			} else {
				d += 2 * (x - y) + 1;
				y -= 1;
			}

			x += 1;

			if x > y {
				break;
			};
		}
	}

	pub fn draw_oro(self: &mut Self, cx: usize, cy: usize) {
		let x = (cx as isize) - 50;
		let y = (cy as isize) - 50;

		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 30);
		let old_color = self.color;
		self.set_color(0, 0, 0, 0);
		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 26);
		self.color = old_color;

		for deg in -130..165 {
			let rad = ((deg % 360) as f32) * 0.01745329252;
			let px = ((rad + 0.9).cos() * 35.0).floor();
			let py = (rad.sin() * 35.0).floor();
			self.mark(
				(x + (50.0 + px) as isize) as usize,
				(y + (50.0 + py) as isize) as usize,
			);
		}

		self.mark_circle_fill((x + 77) as usize, (y + 40) as usize, 11);
		let old_color = self.color;
		self.set_color(0, 0, 0, 0);
		self.mark_circle_fill((x + 77) as usize, (y + 40) as usize, 7);
		self.color = old_color;
	}

	pub fn mark_box(self: &mut Self, x: usize, y: usize, x2: usize, y2: usize) {
		for px in x..x2 {
			self.mark(px, y);
			self.mark(px, y2);
		}
		for py in y..y2 {
			self.mark(x, py);
			self.mark(x2, py);
		}
		self.mark(x2, y2);
	}

	pub fn draw_frame(self: &mut Self) {
		let right = self.info.width - 5;
		let bottom = self.info.height - 5;

		self.mark_box(5, 5, right, bottom);

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

	pub fn mark_glyph(self: &mut Self, glyph: usize, x: usize, y: usize) {
		let glyph_row_offset = FONT_GLYPH_WIDTH * glyph;
		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;
			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + glyph_row_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;
				if ((FONT_BITS[byte] >> (7 - bit)) & 1) == 1 {
					self.mark(x + bx, y + by);
				}
			}
		}
	}
}
