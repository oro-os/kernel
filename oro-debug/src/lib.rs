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
	#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
	oro_debug_pl011::init(offset);
	#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
	oro_debug_uart16550::init();
}

/// Initializes the debug logger with a memory offset of zero.
///
/// The same as calling `init_with_offset(0)`.
#[cfg(debug_assertions)]
pub fn init() {
	init_with_offset(0);
}

/// Logs a message to the debug logger.
///
/// Shouldn't be used directly; use the `dbg!` macros instead.
#[allow(unused_variables)]
pub fn log(message: core::fmt::Arguments) {
	#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
	oro_debug_pl011::log(message);
	#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
	oro_debug_uart16550::log(message);
}

/// Sends a general debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg {
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:I:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg_err {
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:E:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg_warn {
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:W:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}
