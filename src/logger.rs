//! Logging facilities for the boot process, handling
//! both the graphical (frame-buffer) case as well as
//! writing to any serial logger provided by the underlying
//! architecture implementation (e.g. [`crate::arch::SerialLogger`]).

// TODO handle text-mode loggers (when the framebuffer is not allocated
// TODO by the bootloader)

use crate::arch::SerialLogger;
use crate::gfx;
use core::cmp::min;
use core::fmt;

/// The maximum number of supported logger rows
const MAX_LOGGER_ROWS: usize = 256;
/// The maximum number of supported logger columns
const MAX_LOGGER_COLS: usize = 128;

/// The framebuffer logger buffer that stores logger
/// characters for (re-)drawing as the logger is
/// printed to
static mut FRAMEBUFFER_LOGGER_BUFFER: [[u8; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS] =
	[[0; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS];

/// The [`FrameBufferLogger`] instance, if a framebuffer has been
/// provided by the bootloader and registered via [`init_global_framebuffer_logger`]
static mut GLOBAL_FRAMEBUFFER_LOGGER: Option<FrameBufferLogger> = None;
/// The [`SerialLogger`] instance, if applicable, provided by the architecture
/// implementation
static mut GLOBAL_SERIAL_LOGGER: Option<SerialLogger> = None;

/// A logger instance that renders log text to a screen framebuffer,
/// by way of a [`crate::gfx::Rasterizer`]
///
/// # Unsafe
///
/// Do not create multiple instances of this struct with the same
/// underlying [`Self::buffer`].
///
/// This constraint is not checked.
struct FrameBufferLogger {
	/// The x-offset, in pixels, of the left-most character
	x: usize,
	/// The y-offset, in pixels, of the top-most character
	y: usize,
	/// The x-offset, in pixels, of the right-most+1 character
	x2: usize,
	/// The number of rows available to the logger
	///
	/// Even if the framebuffer is larger than expected, the maximum
	/// is capped to [`MAX_LOGGER_ROWS`]
	rows: usize,
	/// The number of columns available to the logger
	///
	/// Even if the framebuffer is larger than expected, the maximum
	/// is capped to [`MAX_LOGGER_COLS`]
	cols: usize,
	/// The underlying character buffer for the logger
	///
	/// # Implementation Note
	///
	/// Due to stack size limitations, the underlying buffer is allocated
	/// statically and assigned here, since this class is only ever
	/// used once under normal and proper conditions.
	// FIXME: This is ugly. Perhaps we only initialize the FrameBufferLogger
	// FIXME: after the architecture is set up, and rely on serial debugging
	// FIXME: for early-stage failures since they're rare. This would allow
	// FIXME: us to defer initialization of this struct until after kernel-space
	// FIXME: heap allocations are established (by way of the global allocator),
	// FIXME: which can then be used to initialize this buffer and we no longer
	// FIXME: have to statically allocate it and mark a bunch of stuff as 'unsafe'.
	buffer: &'static mut [[u8; MAX_LOGGER_COLS]; MAX_LOGGER_ROWS],
	/// The rasterizer to use when drawing
	rasterizer: gfx::Rasterizer,
	/// The current cursor position
	cursor: (usize, usize),
}

impl FrameBufferLogger {
	/// Draw a single character to the rasterizer at the given glyph position
	fn mark_char(&self, x: usize, y: usize, c: u8) {
		self.rasterizer.draw_char_opaque(
			self.x + gfx::GLYPH_WIDTH * x,
			self.y + gfx::GLYPH_HEIGHT * y,
			c,
		);
	}

	/// Shift the entire logger buffer up a line, re-draw all previous
	/// lines to the rasterizer, and clear the last line
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

	/// Write a character to the buffer at the current cursor position,
	/// advancing the cursor, and then draw the new character to the buffer,
	/// shifting up if necessary
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
	#[doc(hidden)]
	fn write_str(&mut self, s: &str) -> fmt::Result {
		for c in s.bytes() {
			self.write_char(c);
		}
		Ok(())
	}
}

/// Initialize the global framebuffer logger instance
///
/// # Arguments
///
/// * `x` - the left offset, in pixels, of the logger area
/// * `y` - the top offset, in pixels, of the logger area
/// * `x2` - the right offset, in pixels, of the logger area
/// * `y2` - the bottom offset, in pixels, of the logger area
/// * `rasterizer` - the [`crate::gfx::Rasterizer`] instance to draw to
///
/// The number of available rows and columns of actual glyphs is
/// calculated based on the logger area size calculated by `x`, `y`,
/// `x2` and `y2` divided by the font's glyph width/height, and
/// capped at [`MAX_LOGGER_ROWS`] and [`MAX_LOGGER_COLS`], respectively.
///
/// # Unsafe
///
/// MUST only ever be called ONCE. Two successful calls
/// to this function induce undefined behavior.
///
/// This constraint is checked in debug builds.
pub unsafe fn init_global_framebuffer_logger(
	x: usize,
	y: usize,
	x2: usize,
	y2: usize,
	rasterizer: gfx::Rasterizer,
) {
	#[cfg(debug_assertions)]
	{
		use core::sync::atomic::{AtomicBool, Ordering};
		#[doc(hidden)]
		static CALLED: AtomicBool = AtomicBool::new(false);
		if CALLED
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_err()
		{
			panic!("must only call init_global_framebuffer_logger() once!");
		}
	}

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

/// Set the global [`crate::arch::SerialLogger`], which is
/// architecture-independent.
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

#[doc(hidden)]
#[macro_export]
macro_rules! println {
	() => (print!("\n"));
	($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
#[macro_export]
macro_rules! print {
	($($arg:tt)*) => ($crate::logger::_print_log(format_args!($($arg)*)));
}
