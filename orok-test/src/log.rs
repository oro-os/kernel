//! Implements log macros for the kernel.

/// Do NOT use this directly; use the `info!`, `debug!`, etc. logging
/// macros instead!
#[cfg(all(feature = "mmio", feature = "emit"))]
#[repr(C, align(16))]
pub struct LogWriter {
	buffer: [u64; 7],
	cursor: usize,
	log:    u64,
}

#[cfg(all(feature = "mmio", feature = "emit"))]
impl LogWriter {
	/// **Do not use.** This is an internal method.
	pub fn new(log: u64) -> Self {
		Self {
			log,
			buffer: [0; 7],
			cursor: 0,
		}
	}

	fn flush(&mut self, message: u64) {
		// SAFETY: We can guarantee we have exactly the right amount of bytes.
		crate::emit_raw!(
			message,
			self.buffer[0],
			self.buffer[1],
			self.buffer[2],
			self.buffer[3],
			self.buffer[4],
			self.buffer[5],
			self.buffer[6],
		);

		self.buffer.fill(0);
		self.cursor = 0;
	}

	fn write_byte(&mut self, b: u8) {
		if self.cursor >= (7 * 8) {
			self.flush(orok_test_consts::LOG);
		}

		// SAFETY: We know this is a valid byte stream and that
		// SAFETY: cursor will never overflow.
		let byte_stream: &mut [u8; 7 * 8] = unsafe { &mut *(&mut self.buffer).as_mut_ptr().cast() };

		byte_stream[self.cursor] = b;
		self.cursor += 1;
	}

	/// **Do not use.** This is an internal method.
	pub fn finish(mut self) {
		self.flush(self.log);
	}
}

#[cfg(all(feature = "mmio", feature = "emit"))]
impl ::core::fmt::Write for LogWriter {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		for b in s.bytes() {
			self.write_byte(b);
		}
		Ok(())
	}
}

/// Log a message with the given constants.
///
/// **Do not use.** this is an internal macro.
#[macro_export]
#[cfg(all(feature = "mmio", feature = "emit"))]
macro_rules! log_with_level {
	($level:ident => { $($tt:tt)* }) => {
		#[cfg(debug_assertions)]
		{
			let mut logger = $crate::log::LogWriter::new(
				$crate::consts::$level,
			);

			core::fmt::write(&mut logger, format_args!($($tt)*)).ok();
			logger.finish();
		}
	}
}

/// Log a message with the given constants.
///
/// **Do not use.** this is an internal macro.
#[macro_export]
#[cfg(not(all(feature = "mmio", feature = "emit")))]
macro_rules! log_with_level {
	($($tt:tt)*) => {};
}

/// Logs a TRACE level message to the debugging stream.
#[macro_export]
macro_rules! trace {
	($($tt:tt)*) => {
		$crate::log_with_level!(LOG_TRACE => { $($tt)* });
	}
}

/// Logs a DEBUG level message to the debugging stream.
#[macro_export]
macro_rules! debug {
	($($tt:tt)*) => {
		$crate::log_with_level!(LOG_DEBUG => { $($tt)* });
	}
}

/// Logs an INFO level message to the debugging stream.
#[macro_export]
macro_rules! info {
	($($tt:tt)*) => {
		$crate::log_with_level!(LOG_INFO => { $($tt)* });
	}
}

/// Logs a WARN level message to the debugging stream.
#[macro_export]
macro_rules! warn {
	($($tt:tt)*) => {
		$crate::log_with_level!(LOG_WARN => { $($tt)* });
	}
}

/// Logs an ERROR level message to the debugging stream.
#[macro_export]
macro_rules! error {
	($($tt:tt)*) => {
		$crate::log_with_level!(LOG_ERROR => { $($tt)* });
	}
}
