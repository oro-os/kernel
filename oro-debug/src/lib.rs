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

/// Initializes the debug logger with a linear map offset, if one is enabled.
///
/// The linear offset is used for debugging backends that use MMIO
/// that gets remapped somewhere else in memory during boot.
#[allow(unused_variables)]
#[cfg(debug_assertions)]
pub fn init_with_offset(offset: usize) {
	#[cfg(feature = "kernel-debug")]
	{
		#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
		oro_debug_pl011::init(offset);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		oro_debug_uart16550::init();
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
#[allow(unused_variables, dead_code)]
pub fn log_debug_bytes(line: &[u8]) {
	#[cfg(feature = "kernel-debug")]
	{
		#[doc(hidden)]
		const PREFIX: &str = "<module>:0:D:";
		#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
		oro_debug_pl011::log_debug_bytes(PREFIX, line);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		oro_debug_uart16550::log_debug_bytes(PREFIX, line);
	}
}

/// Logs a message to the debug logger.
///
/// Shouldn't be used directly; use the `dbg!` macros instead.
#[allow(unused_variables)]
pub fn log(message: core::fmt::Arguments<'_>) {
	#[cfg(feature = "kernel-debug")]
	{
		#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
		oro_debug_pl011::log(message);
		#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
		oro_debug_uart16550::log(message);
	}

	#[cfg(not(feature = "kernel-debug"))]
	{
		let _ = message;
	}
}

/// Sends a general debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg {
	($($arg:tt)*) => {{
		{
			$crate::log(format_args!("{}:{}:I:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_err {
	($($arg:tt)*) => {{
		{
			$crate::log(format_args!("{}:{}:E:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_warn {
	($($arg:tt)*) => {{
		{
			$crate::log(format_args!("{}:{}:W:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
		}
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
			#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
			oro_debug_pl011::log_str_raw(s);
			#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
			oro_debug_uart16550::log_str_raw(s);
		}

		#[cfg(not(feature = "kernel-debug"))]
		{
			let _ = s;
		}

		Ok(())
	}
}
