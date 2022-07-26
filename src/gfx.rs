use core::cell::UnsafeCell;
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

pub type PixelColor = [u8; 8];

pub struct RasterizerInfo {
	pub format: PixelFormat,
	pub width: usize,
	pub height: usize,
	pub stride: usize,
	pub pixel_stride: usize,
}

pub struct Rasterizer {
	info: RasterizerInfo,
	fg_color: PixelColor,
	bg_color: PixelColor,
	pixel_size: usize,
	buffer: UnsafeCell<&'static mut [u8]>,
}

fn set_color(
	format: &PixelFormat,
	pixel_size: usize,
	color: &mut PixelColor,
	r: u8,
	g: u8,
	b: u8,
	grey: u8,
) {
	match format {
		PixelFormat::RGB8 => {
			color[0] = r;
			color[1] = g;
			color[2] = b;
		}
		PixelFormat::BGR8 => {
			color[0] = b;
			color[1] = g;
			color[2] = r;
		}
		PixelFormat::GREY8 => {
			color[0] = grey;
			color[1] = grey;
			color[2] = grey;
		}
		PixelFormat::FALLBACK => {
			for i in 0..pixel_size {
				color[i] = grey;
			}
		}
	};
}

impl Rasterizer {
	pub fn new(buffer: UnsafeCell<&'static mut [u8]>, info: RasterizerInfo) -> Self {
		let pixel_size = match info.format {
			PixelFormat::RGB8 => 3,
			PixelFormat::BGR8 => 3,
			PixelFormat::GREY8 => 1,
			PixelFormat::FALLBACK => min(info.pixel_stride, 8),
		};

		Self {
			info: info,
			fg_color: [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
			bg_color: [0, 0, 0, 0, 0, 0, 0, 0],
			buffer: buffer,
			pixel_size: pixel_size,
		}
	}

	pub fn set_fg(self: &mut Self, r: u8, g: u8, b: u8, grey: u8) {
		set_color(
			&self.info.format,
			self.pixel_size,
			&mut self.fg_color,
			r,
			g,
			b,
			grey,
		);
	}

	pub fn set_bg(self: &mut Self, r: u8, g: u8, b: u8, grey: u8) {
		set_color(
			&self.info.format,
			self.pixel_size,
			&mut self.fg_color,
			r,
			g,
			b,
			grey,
		);
	}

	pub fn clear(self: &Self) {
		for y in 0..self.info.height {
			for x in 0..self.info.width {
				self.mark_unsafe(x, y, &self.bg_color);
			}
		}
	}

	fn mark_circle_outline(self: &Self, cx: usize, cy: usize, r: usize, color: &PixelColor) {
		let mut d = (5 - (r as isize) * 4) / 4;
		let mut x = 0 as isize;
		let mut y = r as isize;

		let cxi = cx as isize;
		let cyi = cy as isize;

		loop {
			self.mark((cxi + x) as usize, (cyi + y) as usize, color);
			self.mark((cxi + x) as usize, (cyi - y) as usize, color);
			self.mark((cxi - x) as usize, (cyi + y) as usize, color);
			self.mark((cxi - x) as usize, (cyi - y) as usize, color);
			self.mark((cxi + y) as usize, (cyi + x) as usize, color);
			self.mark((cxi + y) as usize, (cyi - x) as usize, color);
			self.mark((cxi - y) as usize, (cyi + x) as usize, color);
			self.mark((cxi - y) as usize, (cyi - x) as usize, color);

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

	fn mark_line_to_y(self: &Self, x: usize, y: usize, to_y: usize, color: &PixelColor) {
		if y < to_y {
			for py in y..to_y {
				self.mark(x, py, color);
			}
		} else {
			for py in to_y..y {
				self.mark(x, py, color);
			}
		}
	}

	fn mark_circle_fill(self: &Self, cx: usize, cy: usize, r: usize, color: &PixelColor) {
		let mut d = (5 - (r as isize) * 4) / 4;
		let mut x = 0 as isize;
		let mut y = r as isize;

		let cxi = cx as isize;
		let cyi = cy as isize;

		loop {
			self.mark_line_to_y((cxi + x) as usize, (cyi + y) as usize, cy, color);
			self.mark_line_to_y((cxi + x) as usize, (cyi - y) as usize, cy, color);
			self.mark_line_to_y((cxi - x) as usize, (cyi + y) as usize, cy, color);
			self.mark_line_to_y((cxi - x) as usize, (cyi - y) as usize, cy, color);
			self.mark_line_to_y((cxi + y) as usize, (cyi + x) as usize, cy, color);
			self.mark_line_to_y((cxi + y) as usize, (cyi - x) as usize, cy, color);
			self.mark_line_to_y((cxi - y) as usize, (cyi + x) as usize, cy, color);
			self.mark_line_to_y((cxi - y) as usize, (cyi - x) as usize, cy, color);

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

	fn mark_oro(self: &Self, cx: usize, cy: usize, fg: &PixelColor, bg: &PixelColor) {
		let x = (cx as isize) - 50;
		let y = (cy as isize) - 50;

		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 30, fg);
		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 26, bg);

		for deg in -130..165 {
			let rad = ((deg % 360) as f32) * 0.01745329252;
			let px = ((rad + 0.9).cos() * 35.0).floor();
			let py = (rad.sin() * 35.0).floor();

			self.mark(
				(x + (50.0 + px) as isize) as usize,
				(y + (50.0 + py) as isize) as usize,
				fg,
			);
		}

		self.mark_circle_fill((x + 77) as usize, (y + 40) as usize, 11, fg);
		self.mark_circle_fill((x + 77) as usize, (y + 40) as usize, 7, bg);
	}

	pub fn draw_boot_frame(self: &Self) {
		self.mark_box(
			5,
			5,
			self.info.width - 5,
			self.info.height - 5,
			&self.fg_color,
		);

		self.mark_box_fill(
			6,
			self.info.height - 106,
			81,
			self.info.height - 6,
			&self.fg_color,
		);

		self.mark_oro(42, self.info.height - 55, &self.bg_color, &self.fg_color);

		for py in 0..38 {
			for px in 0..(py * 2) {
				self.mark_unsafe(px + 6, py + self.info.height - 106 - 38, &self.fg_color);
			}
		}
	}

	fn mark_box_fill(self: &Self, x: usize, y: usize, x2: usize, y2: usize, color: &PixelColor) {
		if x >= self.info.width || y >= self.info.height {
			return;
		}
		let xr = min(x2, self.info.width - 1);
		let yr = min(y2, self.info.height - 1);
		for py in y..=yr {
			for px in x..=xr {
				self.mark_unsafe(px, py, color);
			}
		}
	}

	fn mark_box(self: &Self, x: usize, y: usize, x2: usize, y2: usize, color: &PixelColor) {
		for px in x..x2 {
			self.mark(px, y, color);
			self.mark(px, y2, color);
		}
		for py in y..y2 {
			self.mark(x, py, color);
			self.mark(x2, py, color);
		}
		self.mark(x2, y2, color);
	}

	fn mark(self: &Self, x: usize, y: usize, color: &PixelColor) {
		if x >= self.info.width || y >= self.info.height {
			return;
		}
		self.mark_unsafe(x, y, color);
	}

	fn mark_unsafe(self: &Self, x: usize, y: usize, color: &PixelColor) {
		let offset = (y * self.info.stride + x) * self.info.pixel_stride;

		for i in 0..self.pixel_size {
			unsafe {
				(*(self.buffer.get()))[offset + i] = color[i];
			}
		}
	}

	fn mark_glyph(self: &Self, glyph: usize, x: usize, y: usize, color: &PixelColor) {
		let glyph_row_offset = FONT_GLYPH_WIDTH * glyph;
		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;
			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + glyph_row_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;
				if ((FONT_BITS[byte] >> (7 - bit)) & 1) == 1 {
					self.mark(x + bx, y + by, color);
				}
			}
		}
	}
}
