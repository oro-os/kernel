//! Implements the rasterizer in cases where the boot sequence
//! is handed a framebuffer. The rasterizer is intended to be
//! architecture agnostic, as long as the buffer it's handed
//! has a reasonable pixel format (including support for pixel
//! and scanline strides).

use const_format::formatcp;
use core::cell::UnsafeCell;
use core::cmp::min;
use micromath::F32Ext;

// This pulls in generated font information and metrics
// from the build step. See build.rs (or wherever Cargo.toml
// indicates where the build script for the Kernel resides)
include!(concat!(env!("OUT_DIR"), "/oro_font.rs"));

/// Re-export of [`FONT_GLYPH_WIDTH`]
pub const GLYPH_WIDTH: usize = FONT_GLYPH_WIDTH;
/// Re-export of [`FONT_GLYPH_HEIGHT`]
pub const GLYPH_HEIGHT: usize = FONT_GLYPH_HEIGHT;
/// The padding around the boot frame, in pixels
pub const PADDING: usize = 10;
/// The width, in pixels, of the left gutter bar,
/// which includes the Oro logo and version information.
pub const LEFT_GUTTER_WIDTH: usize = 90;

/// The pixel format of the underlying graphics buffer
/// used by the boot frame rasterizer.
#[derive(Default)]
pub enum PixelFormat {
	/// u8 x3: R, G, B
	RGB8,
	/// u8 x3: B, G, R
	BGR8,
	/// u8 x1: Grey
	Grey8,
	/// u8 xN: Grey * N
	///
	/// `n` refers to the size of [`PixelColor`]
	/// in the worst case. Typically, a pixel size
	/// is passed into the rasterizer, usually from
	/// information provided by the bootloader.
	#[default]
	Fallback,
}

/// A single pixel color (either background, foreground, or accent color).
///
/// The length of the array is meant to indicate the _maximum_ possible
/// pixel stride across all architectures. This is for use in cases where
/// grey is needed to be used as a fallback and the stride is larger than
/// RGB\[AX].
pub type PixelColor = [u8; 8];

/// High-level rasterizer framebuffer information.
// TODO Rename this from `RasterizerInfo` to `FrameBufferMetrics`
// TODO and remove the excessive "of the underlying framebuffer"
// TODO clauses from the docs below
#[derive(Default)]
pub struct RasterizerInfo {
	/// The pixel format of the underlying framebuffer
	pub format: PixelFormat,
	/// The width, in pixels, of the underlying framebuffer
	pub width: usize,
	/// The height, in pixels, of the underlying framebuffer
	pub height: usize,
	/// The number of bytes per scanline of the underlying framebuffer
	pub stride: usize,
	/// The number of bytes per pixel of the underlying framebuffer
	///
	/// Note that the pixel stride may be greater than the number
	/// of actually used pixels. The rasterizer makes no guarantee
	/// of the values of the unused bytes when drawing. Best case,
	/// they're either set to the 'grey' value or `0`. Worst case,
	/// they are left untouched.
	pub pixel_stride: usize,
}

/// Rasterizes various shapes, lines, simple graphics,
/// and fonts onto a pixel buffer. Meant only for
/// the boot process (i.e. is not exposed or intended
/// for user processes, window servers, etc.)
///
/// Each drawable graphic uses one or more of a total
/// of three colors - the foreground, background, and
/// accent colors - which can be configured on the fly,
/// or set once and re-used across all draw calls.
///
/// This is especially important for some of the more
/// complicated graphics, such as the Oro logo and other
/// "higher order" draw calls.
pub struct Rasterizer {
	/// The high-level rasterizer framebuffer information
	info: RasterizerInfo,
	/// The foreground color to use when drawing
	fg_color: PixelColor,
	/// The background color to use when drawing
	bg_color: PixelColor,
	/// The accent color to use when drawing
	acc_color: PixelColor,
	/// The number of important bytes in a single pixel (_not_ the stride)
	pixel_size: usize,
	/// An underlying reference to the video framebuffer
	buffer: UnsafeCell<&'static mut [u8]>,
}

/// Assigns an RGB/Grey color to a [`PixelColor`] array in the format
/// required by the given [`PixelFormat`]
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
		PixelFormat::Grey8 => {
			color[0] = grey;
			color[1] = grey;
			color[2] = grey;
		}
		#[allow(clippy::needless_range_loop)]
		PixelFormat::Fallback => {
			for i in 0..pixel_size {
				color[i] = grey;
			}
		}
	};
}

impl Rasterizer {
	/// Creates a new rasterizer instance given the framebuffer reference
	/// and framebuffer metrics information
	pub fn new(buffer: UnsafeCell<&'static mut [u8]>, info: RasterizerInfo) -> Self {
		let pixel_size = match info.format {
			PixelFormat::RGB8 => 3,
			PixelFormat::BGR8 => 3,
			PixelFormat::Grey8 => 1,
			PixelFormat::Fallback => min(info.pixel_stride, 8),
		};

		Self {
			info,
			fg_color: [0xFF; 8],
			bg_color: [0; 8],
			acc_color: [0xFF; 8],
			buffer,
			pixel_size,
		}
	}

	/// Set the foreground color used by any following draw calls
	///
	/// Both an RGB value as well as a greyscale value must be
	/// passed, the latter of which is used in cases where the
	/// framebuffer doesn't support color, or when pixel format
	/// is unknown (i.e. [`PixelFormat::Fallback`]).
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

	/// Set the background color used by any following draw calls
	///
	/// Both an RGB value as well as a greyscale value must be
	/// passed, the latter of which is used in cases where the
	/// framebuffer doesn't support color, or when pixel format
	/// is unknown (i.e. [`PixelFormat::Fallback`]).
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

	/// Set the accent color used by any following draw calls
	///
	/// Both an RGB value as well as a greyscale value must be
	/// passed, the latter of which is used in cases where the
	/// framebuffer doesn't support color, or when pixel format
	/// is unknown (i.e. [`PixelFormat::Fallback`]).
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

	/// Set all pixels in the framebuffer to the background color
	pub fn clear_screen(&self) {
		for y in 0..self.info.height {
			for x in 0..self.info.width {
				self.mark_unsafe(x, y, &self.bg_color);
			}
		}
	}

	/// Set all pixels the given rectangle to the background color
	pub fn clear(&self, x: usize, y: usize, x2: usize, y2: usize) {
		let xr = min(x2, self.info.width);
		let yr = min(y2, self.info.height);

		for py in y..yr {
			for px in x..xr {
				self.mark_unsafe(px, py, &self.bg_color);
			}
		}
	}

	#[doc(hidden)]
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

	/// Fill a circle
	fn mark_circle_fill(&self, cx: usize, cy: usize, r: usize, color: &PixelColor) {
		let mut d = (5 - (r as isize) * 4) / 4;
		let mut x = 0_isize;
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

	/// Draw the Oro logo
	///
	/// Note that `cx` and `cy` indicate the center of the logo, _not_ the
	/// top left.
	fn mark_oro(&self, cx: usize, cy: usize, fg: &PixelColor, bg: &PixelColor) {
		let x = (cx as isize) - 50;
		let y = (cy as isize) - 50;

		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 30, fg);
		self.mark_circle_fill((x + 50) as usize, (y + 50) as usize, 26, bg);

		for deg in -130..165 {
			let rad = ((deg % 360) as f32) * 0.017_453_292;
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

		// FIXME: This is a hack to kill that one extra pixel at the bottom
		// FIXME: of the Oro orbit line. If you are familiar with trig and
		// FIXME: know of a way to smooth out the line automatically, that'd
		// FIXME: be awesome.
		self.mark(cx - 28, cy + 35, bg);
	}

	/// Draw the boot frame, within which all logs/logos/etc. are included.
	pub fn draw_boot_frame(&self) {
		self.mark_box(
			PADDING / 2,
			PADDING / 2,
			self.info.width - PADDING / 2,
			self.info.height - PADDING / 2,
			&self.acc_color,
		);

		#[doc(hidden)]
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
			#[doc(hidden)]
			const ORO: &str = "ORO";
			let top = version_top;
			let left = PADDING + (LEFT_GUTTER_WIDTH / 2) - ((ORO.len() * GLYPH_WIDTH) / 2);

			ORO.bytes()
				.enumerate()
				.for_each(|(i, c)| self.draw_char(left + i * GLYPH_WIDTH, top, c));
		}

		// version info
		{
			#[doc(hidden)]
			const VERSION: &str = formatcp!(
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

	/// Draw an outlined rectangle
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

	/// Set the color of a single pixel
	fn mark(&self, x: usize, y: usize, color: &PixelColor) {
		if x >= self.info.width || y >= self.info.height {
			return;
		}
		self.mark_unsafe(x, y, color);
	}

	/// Set the color of a single pixel **without bounds checking**
	///
	/// # Unsafe
	///
	/// Caller MUST take care that the pixel is within the confines
	/// of the frame buffer; otherwise, it runs the risk of writing
	/// beyond the framebuffer end address.
	///
	/// This constraint is checked in debug builds.
	fn mark_unsafe(&self, x: usize, y: usize, color: &PixelColor) {
		let offset = (y * self.info.stride + x) * self.info.pixel_stride;

		debug_assert!(x < self.info.width);
		debug_assert!(y < self.info.height);

		#[allow(clippy::needless_range_loop)]
		for i in 0..self.pixel_size {
			unsafe {
				(*(self.buffer.get()))[offset + i] = color[i];
			}
		}
	}

	/// Draw a single character
	///
	/// Does not paint the background color (i.e. the character
	/// is drawn 'transparently')
	///
	/// Character must exist within the glyph map (see the kernel
	/// build script for a list of all characters), otherwise a
	///  'fallback' character is drawn.
	pub fn draw_char(&self, x: usize, y: usize, c: u8) {
		let lookup = FONT_GLYPH_LOOKUP[c as usize];

		if lookup == 255 {
			self.mark_unknown_glyph(x, y, &self.fg_color);
		} else {
			self.mark_glyph(lookup as usize, x, y, &self.fg_color);
		}
	}

	/// Draw a single character, also painting the background
	///
	/// Character must exist within the glyph map (see the kernel
	/// build script for a list of all characters), otherwise a
	///  'fallback' character is drawn.
	pub fn draw_char_opaque(&self, x: usize, y: usize, c: u8) {
		let lookup = FONT_GLYPH_LOOKUP[c as usize];

		if lookup == 255 {
			self.mark_unknown_glyph_opaque(x, y, &self.fg_color, &self.bg_color);
		} else {
			self.mark_glyph_opaque(lookup as usize, x, y, &self.fg_color, &self.bg_color);
		}
	}

	/// Draw the 'unknown' glyph (glyph 0 from the atlas, inverted)
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

	/// Draw a single glyph to the screen. Glyph must be a valid
	/// offset in [`FONT_BITS`].
	///
	/// # Unsafe
	///
	/// The `glyph` offset MUST be < the total number of glyphs
	/// in the atlas, otherwise arbitrary memory will be read.
	///
	/// This constraint is checked in debug builds.
	fn mark_glyph(&self, glyph: usize, x: usize, y: usize, color: &PixelColor) {
		let glyph_row_offset = FONT_GLYPH_WIDTH * glyph;

		for by in 0..FONT_GLYPH_HEIGHT {
			let bit_offset = by * FONT_GLYPH_STRIDE_BITS;

			for bx in 0..FONT_GLYPH_WIDTH {
				let abs_bit = bit_offset + glyph_row_offset + bx;
				let byte = abs_bit / 8;
				let bit = abs_bit % 8;

				debug_assert!(byte < FONT_BITS.len());

				if ((FONT_BITS[byte] >> (7 - bit)) & 1) == 1 {
					self.mark(x + bx, y + by, color);
				}
			}
		}
	}

	/// Draw the 'unknown' glyph (glypn 0 from the atlas, inverted), also
	/// painting the background
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

	/// Draw a glyph, also drawing the background
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
