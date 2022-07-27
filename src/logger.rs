use crate::gfx;
use core::cmp::min;
use core::fmt;
use core::ptr::null_mut;

const MAX_LOGGER_ROWS: usize = 256;
const MAX_LOGGER_COLS: usize = 128;

pub struct BootLogger {
	x: usize,
	y: usize,
	x2: usize,
	y2: usize,
	rows: usize,
	cols: usize,
	rasterizer: gfx::Rasterizer,
	buffer: [[u8; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS],
	cursor: (usize, usize),
}

impl BootLogger {
	pub fn new(x: usize, y: usize, x2: usize, y2: usize, rasterizer: gfx::Rasterizer) -> Self {
		let cols = min(MAX_LOGGER_COLS, (x2 - x) / gfx::GLYPH_WIDTH);
		let rows = min(MAX_LOGGER_ROWS, (y2 - y) / gfx::GLYPH_HEIGHT);

		Self {
			x: x,
			y: y,
			rasterizer: rasterizer,
			buffer: [[0; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS],
			cursor: (0, 0),
			x2: x + cols * gfx::GLYPH_WIDTH,
			y2: y + rows * gfx::GLYPH_HEIGHT,
			rows: rows,
			cols: cols,
		}
	}

	fn mark_char(&self, x: usize, y: usize, c: u8) {
		self.rasterizer.draw_char(
			self.x + gfx::GLYPH_WIDTH * x,
			self.y + gfx::GLYPH_HEIGHT * y,
			c,
		);
	}

	fn redraw(&self) {
		self.rasterizer.clear(self.x, self.y, self.x2, self.y2);

		for y in 0..self.rows {
			let row = &self.buffer[y];

			for x in 0..self.cols {
				let c = row[x];

				if c == 0 {
					break;
				} else {
					self.mark_char(x, y, c);
				}
			}
		}
	}

	fn shift_up(&mut self) {
		for i in 1..self.rows {
			self.buffer[i - 1] = self.buffer[i];
		}

		self.buffer[self.rows - 1] = [0; MAX_LOGGER_COLS];

		self.redraw();
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

		if c == ('\n' as u8) {
			self.cursor.0 = 0;
			self.cursor.1 += 1;
		} else {
			self.buffer[self.cursor.1][self.cursor.0] = c;
			self.mark_char(self.cursor.0, self.cursor.1, c);
			self.cursor.0 += 1;
		}
	}
}

impl fmt::Write for BootLogger {
	fn write_str(&mut self, s: &str) -> fmt::Result {
		for c in s.bytes() {
			self.write_char(c);
		}
		Ok(())
	}
}

static mut GLOBAL_LOGGER: *mut BootLogger = null_mut();

pub fn set_global_logger(logger: *mut BootLogger) {
	unsafe {
		GLOBAL_LOGGER = logger;
	}
}

#[doc(hidden)]
pub fn _print_log(args: fmt::Arguments) {
	use fmt::Write;
	if let Some(logger) = unsafe { GLOBAL_LOGGER.as_mut() } {
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
