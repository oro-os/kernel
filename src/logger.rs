use crate::arch::SerialLogger;
use crate::gfx;
use core::cmp::min;
use core::fmt;

const MAX_LOGGER_ROWS: usize = 256;
const MAX_LOGGER_COLS: usize = 128;

static mut FRAMEBUFFER_LOGGER_BUFFER: [[u8; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS] =
	[[0; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS];

static mut GLOBAL_FRAMEBUFFER_LOGGER: Option<FrameBufferLogger> = None;
static mut GLOBAL_SERIAL_LOGGER: Option<SerialLogger> = None;

struct FrameBufferLogger {
	x: usize,
	y: usize,
	x2: usize,
	rows: usize,
	cols: usize,
	buffer: &'static mut [[u8; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS],
	rasterizer: gfx::Rasterizer,
	cursor: (usize, usize),
}

impl FrameBufferLogger {
	fn mark_char(&self, x: usize, y: usize, c: u8) {
		self.rasterizer.draw_char_opaque(
			self.x + gfx::GLYPH_WIDTH * x,
			self.y + gfx::GLYPH_HEIGHT * y,
			c,
		);
	}

	fn shift_up(&mut self) {
		for y in 0..(self.rows - 1) {
			let row = &self.buffer[y + 1];

			#[allow(clippy::needless_range_loop)]
			for x in 0..self.cols {
				let c = row[x];

				if c == 0 {
					let top = self.y + y * gfx::GLYPH_HEIGHT;

					self.rasterizer.clear(
						self.x + x * gfx::GLYPH_WIDTH,
						top,
						self.x2,
						top + gfx::GLYPH_HEIGHT,
					);

					break;
				} else {
					self.mark_char(x, y, c);
				}
			}

			self.buffer[y] = *row;
		}

		let top = self.y + (self.rows - 1) * gfx::GLYPH_HEIGHT;
		self.rasterizer
			.clear(self.x, top, self.x2, top + gfx::GLYPH_HEIGHT);

		self.buffer[self.rows - 1] = [0; MAX_LOGGER_COLS];
	}

	fn write_char(&mut self, c: u8) {
		if self.cursor.0 == self.cols {
			self.cursor.0 = 0;
			self.cursor.1 += 1;
		}

		if self.cursor.1 == self.rows {
			self.shift_up();
			self.cursor.1 = self.rows - 1;
		}

		if c == b'\n' {
			self.cursor.0 = 0;
			self.cursor.1 += 1;
		} else {
			self.buffer[self.cursor.1][self.cursor.0] = c;
			self.mark_char(self.cursor.0, self.cursor.1, c);
			self.cursor.0 += 1;
		}
	}
}

impl fmt::Write for FrameBufferLogger {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		for c in s.bytes() {
			self.write_char(c);
		}
		Ok(())
	}
}

/*
	Unsafe because it MUST only ever be called once!
*/
pub unsafe fn init_global_framebuffer_logger(
	x: usize,
	y: usize,
	x2: usize,
	y2: usize,
	rasterizer: gfx::Rasterizer,
) {
	let cols = min(MAX_LOGGER_COLS, (x2 - x) / gfx::GLYPH_WIDTH);
	let rows = min(MAX_LOGGER_ROWS, (y2 - y) / gfx::GLYPH_HEIGHT);

	let res = FrameBufferLogger {
		x,
		y,
		rasterizer,
		buffer: &mut FRAMEBUFFER_LOGGER_BUFFER,
		cursor: (0, 0),
		x2: x + cols * gfx::GLYPH_WIDTH,
		rows,
		cols,
	};

	GLOBAL_FRAMEBUFFER_LOGGER = Some(res);
}

pub fn set_global_serial_logger(logger: SerialLogger) {
	unsafe {
		GLOBAL_SERIAL_LOGGER = Some(logger);
	}
}

#[doc(hidden)]
pub fn _print_log(args: fmt::Arguments) {
	use fmt::Write;

	if let Some(logger) = unsafe { GLOBAL_SERIAL_LOGGER.as_mut() } {
		logger.write_fmt(args).unwrap();
	}

	if let Some(logger) = unsafe { GLOBAL_FRAMEBUFFER_LOGGER.as_mut() } {
		logger.write_fmt(args).unwrap();
	}
}

#[macro_export]
macro_rules! println {
	() => (print!("\n"));
	($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
	($($arg:tt)*) => ($crate::logger::_print_log(format_args!($($arg)*)));
}
