use const_format::formatcp;
use core::cell::UnsafeCell;
use core::cmp::min;
use micromath::F32Ext;

include!(concat!(env!("OUT_DIR"), "/oro_font.rs"));

pub const GLYPH_WIDTH: usize = FONT_GLYPH_WIDTH;
pub const GLYPH_HEIGHT: usize = FONT_GLYPH_HEIGHT;
pub const PADDING: usize = 10;
pub const LEFT_GUTTER_WIDTH: usize = 90;

#[derive(Default)]
pub enum PixelFormat {
	RGB8,
	BGR8,
	GREY8,
	#[default]
	FALLBACK,
}

pub type PixelColor = [u8; 8];

#[derive(Default)]
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
	acc_color: PixelColor,
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
			fg_color: [0xFF; 8],
			bg_color: [0; 8],
			acc_color: [0xFF; 8],
			buffer: buffer,
			pixel_size: pixel_size,
		}
	}

	pub fn set_fg(&mut self, r: u8, g: u8, b: u8, grey: u8) {
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

	pub fn set_bg(&mut self, r: u8, g: u8, b: u8, grey: u8) {
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

	pub fn set_accent(&mut self, r: u8, g: u8, b: u8, grey: u8) {
		set_color(
			&self.info.format,
			self.pixel_size,
			&mut self.acc_color,
			r,
			g,
			b,
			grey,
		);
	}

	pub fn clear_screen(&self) {
		for y in 0..self.info.height {
			for x in 0..self.info.width {
				self.mark_unsafe(x, y, &self.bg_color);
			}
		}
	}

	pub fn clear(&self, x: usize, y: usize, x2: usize, y2: usize) {
		let xr = min(x2, self.info.width);
		let yr = min(y2, self.info.height);

		for py in y..yr {
			for px in x..xr {
				self.mark_unsafe(px, py, &self.bg_color);
			}
		}
	}

	fn mark_line_to_y(&self, x: usize, y: usize, to_y: usize, color: &PixelColor) {
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

	fn mark_circle_fill(&self, cx: usize, cy: usize, r: usize, color: &PixelColor) {
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

	fn mark_oro(&self, cx: usize, cy: usize, fg: &PixelColor, bg: &PixelColor) {
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

	pub fn draw_boot_frame(&self) {
		self.mark_box(
			PADDING / 2,
			PADDING / 2,
			self.info.width - PADDING / 2,
			self.info.height - PADDING / 2,
			&self.acc_color,
		);

		const ORO_X_MANUAL_TRANSLATE: usize = 3;
		self.mark_oro(
			PADDING + LEFT_GUTTER_WIDTH / 2 - ORO_X_MANUAL_TRANSLATE,
			self.info.height - (4 * GLYPH_HEIGHT + PADDING) - 50,
			&self.fg_color,
			&self.bg_color,
		);

		let version_top = self.info.height - (4 * GLYPH_HEIGHT + PADDING);

		// "ORO"
		{
			const ORO: &'static str = "ORO";
			let top = version_top;
			let left = PADDING + (LEFT_GUTTER_WIDTH / 2) - ((ORO.len() * GLYPH_WIDTH) / 2);

			ORO.bytes()
				.enumerate()
				.for_each(|(i, c)| self.draw_char(left + i * GLYPH_WIDTH, top, c));
		}

		// version info
		{
			const VERSION: &'static str = formatcp!(
				"{}-{}",
				env!("CARGO_PKG_VERSION"),
				if cfg!(debug_assertions) { "d" } else { "r" }
			);

			let top = version_top + GLYPH_HEIGHT;
			let left = PADDING + (LEFT_GUTTER_WIDTH / 2) - ((VERSION.len() * GLYPH_WIDTH) / 2);

			VERSION
				.bytes()
				.enumerate()
				.for_each(|(i, c)| self.draw_char(left + i * GLYPH_WIDTH, top, c));
		}
	}

	fn mark_box(&self, x: usize, y: usize, x2: usize, y2: usize, color: &PixelColor) {
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

	fn mark(&self, x: usize, y: usize, color: &PixelColor) {
		if x >= self.info.width || y >= self.info.height {
			return;
		}
		self.mark_unsafe(x, y, color);
	}

	fn mark_unsafe(&self, x: usize, y: usize, color: &PixelColor) {
		let offset = (y * self.info.stride + x) * self.info.pixel_stride;

		for i in 0..self.pixel_size {
			unsafe {
				(*(self.buffer.get()))[offset + i] = color[i];
			}
		}
	}

	pub fn draw_char(&self, x: usize, y: usize, c: u8) {
		let lookup = FONT_GLYPH_LOOKUP[c as usize];

		if lookup == 255 {
			self.mark_unknown_glyph(x, y, &self.fg_color);
		} else {
			self.mark_glyph(lookup as usize, x, y, &self.fg_color);
		}
	}

	pub fn draw_char_opaque(&self, x: usize, y: usize, c: u8) {
		let lookup = FONT_GLYPH_LOOKUP[c as usize];

		if lookup == 255 {
			self.mark_unknown_glyph_opaque(x, y, &self.fg_color, &self.bg_color);
		} else {
			self.mark_glyph_opaque(lookup as usize, x, y, &self.fg_color, &self.bg_color);
		}
	}

	fn mark_unknown_glyph(&self, x: usize, y: usize, color: &PixelColor) {
		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;

			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;

				if ((FONT_BITS[byte] >> (7 - bit)) & 1) == 0 {
					self.mark(x + bx, y + by, color);
				}
			}
		}
	}

	fn mark_glyph(&self, glyph: usize, x: usize, y: usize, color: &PixelColor) {
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

	fn mark_unknown_glyph_opaque(
		&self,
		x: usize,
		y: usize,
		color: &PixelColor,
		bg_color: &PixelColor,
	) {
		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;

			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;

				let v = ((FONT_BITS[byte] >> (7 - bit)) & 1) == 0;
				self.mark(x + bx, y + by, if v { color } else { bg_color });
			}
		}
	}

	fn mark_glyph_opaque(
		&self,
		glyph: usize,
		x: usize,
		y: usize,
		color: &PixelColor,
		bg_color: &PixelColor,
	) {
		let glyph_row_offset = FONT_GLYPH_WIDTH * glyph;

		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;

			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + glyph_row_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;

				let v = ((FONT_BITS[byte] >> (7 - bit)) & 1) == 1;
				self.mark(x + bx, y + by, if v { color } else { bg_color });
			}
		}
	}
}
