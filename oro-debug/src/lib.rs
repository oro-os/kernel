//! Debugging utilities for the Oro kernel.
//!
//! Implements a wrapper around various serial output
//! mechanism for early-stage logging.
//!
//! **IMPORTANT:** This crate is not very robust, and is
//! not intended to be used in production (release builds).
//! Namely, it's not interrupt-safe and may cause deadlocks
//! if used improperly.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
#![expect(internal_features)]
#![feature(core_intrinsics)]

use oro_sync::{Lock, TicketMutex};

#[cfg(any(doc, all(debug_assertions, feature = "pl011")))]
mod pl011;
mod ringbuffer;
#[cfg(any(
	doc,
	all(debug_assertions, target_arch = "x86_64", feature = "uart16550")
))]
mod uart16550;

/// The size of the internal ring buffer.
const RING_BUFFER_SIZE: usize = 4096;

/// Static ring buffer that receives all bytes logged.
static RING_BUFFER: TicketMutex<ringbuffer::RingBuffer<RING_BUFFER_SIZE>> =
	TicketMutex::new(ringbuffer::RingBuffer::new());

/// Returns the length of the internal ring buffer used for
/// retrieving log data by e.g. the boot logger.
#[must_use]
pub const fn ring_buffer_len() -> usize {
	RING_BUFFER_SIZE
}

/// Reads a single `u64` from the ring buffer.
///
/// See [`ringbuffer::RingBuffer::read()`] for more information.
#[must_use]
#[inline]
pub fn ring_buffer_read() -> u64 {
	RING_BUFFER.lock().read()
}

/// Initializes the debug logger with a linear map offset, if one is enabled.
///
/// The linear offset is used for debugging backends that use MMIO
/// that gets remapped somewhere else in memory during boot.
#[allow(unused_variables)]
#[cfg(debug_assertions)]
pub fn init_with_offset(offset: usize) {
	#[cfg(feature = "kernel-debug")]
	{
		#[cfg(feature = "pl011")]
		self::pl011::init(offset);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		self::uart16550::init();
	}
}

/// Initializes the debug logger with a memory offset of zero.
///
/// The same as calling `init_with_offset(0)`.
#[cfg(debug_assertions)]
pub fn init() {
	init_with_offset(0);
}

/// Logs a module-level debug line to the debug logger.
///
/// To be used only by the root-ring kernel debug output interface.
#[cfg(debug_assertions)]
pub fn log_debug_bytes(line: &[u8]) {
	#[cfg(feature = "kernel-debug")]
	{
		#[allow(dead_code)]
		#[doc(hidden)]
		const PREFIX: &str = "D:<module>:0:";
		#[cfg(feature = "pl011")]
		self::pl011::log_debug_bytes(PREFIX, line);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		self::uart16550::log_debug_bytes(PREFIX, line);
	}

	let mut buf = RING_BUFFER.lock();
	buf.write(b"  ");
	buf.write(line);
	buf.write(b"\n");
	drop(buf);
}

/// Logs a message to the debug logger.
///
/// Shouldn't be used directly; use the `dbg!` macros instead.
#[allow(unused_variables)]
#[cfg(debug_assertions)]
pub fn log(message: core::fmt::Arguments<'_>) {
	#[cfg(feature = "kernel-debug")]
	{
		#[cfg(feature = "pl011")]
		self::pl011::log(message);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		self::uart16550::log(message);
	}
}

/// Logs a message directly to the ring buffer.
///
/// Shouldn't be used directly; use the `dbg!` macros instead.
pub fn log_ring_fmt(message: core::fmt::Arguments<'_>) {
	use core::fmt::Write;
	let mut buf = RING_BUFFER.lock();
	let _ = buf.write_fmt(message);
	buf.write(b"\n");
	drop(buf);
}

/// Sends a general debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg {
	($($arg:tt)*) => {{
		#[cfg(debug_assertions)]
		{
			$crate::log(format_args!("I:{}:{}:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}

		$crate::log_ring_fmt(format_args!("  {}", format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_err {
	($($arg:tt)*) => {{
		#[cfg(debug_assertions)]
		{
			$crate::log(format_args!("E:{}:{}:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}

		$crate::log_ring_fmt(format_args!("? {}", format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_warn {
	($($arg:tt)*) => {{
		#[cfg(debug_assertions)]
		{
			$crate::log(format_args!("W:{}:{}:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}

		$crate::log_ring_fmt(format_args!("! {}", format_args!($($arg)*)));
	}};
}

/// A `fmt::Write` logger. Used primarily for low-level debugging.
/// Please use sparingly, and prefer the `dbg!` macro for most
/// debugging needs.
#[cfg(debug_assertions)]
pub struct DebugWriter;

#[cfg(debug_assertions)]
impl core::fmt::Write for DebugWriter {
	#[allow(unused_variables)]
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		#[cfg(feature = "kernel-debug")]
		{
			#[cfg(feature = "pl011")]
			self::pl011::log_str_raw(s);
			#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
			self::uart16550::log_str_raw(s);
		}

		let _ = RING_BUFFER.lock().write_str(s);

		Ok(())
	}
}
