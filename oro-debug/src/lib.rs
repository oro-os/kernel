//! Debugging utilities for the Oro kernel.
//!
//! Implements a wrapper around various serial output
//! mechanism for early-stage logging, as well as
//! a few utilities for debugging the kernel via GDB
//! (e.g. the dbgutil stubs).
//!
//! **IMPORTANT:** This crate is not very robust, and is
//! not intended to be used in production (release builds).
//! Namely, it's not interrupt-safe and may cause deadlocks
//! if used improperly.
#![cfg_attr(not(test), no_std)]

#[cfg(any(debug_assertions, feature = "dbgutil"))]
use oro_common_proc::gdb_autoload_inline;

#[cfg(any(debug_assertions, feature = "dbgutil"))]
gdb_autoload_inline!("dbgutil.py");

/// Initializes the debug logger, if one is enabled.
#[cfg(debug_assertions)]
pub fn init() {
	#[cfg(all(target_arch = "aarch64", feature = "pl011"))]
	oro_debug_pl011::init();
	#[cfg(all(target_arch = "x86_64", feature = "uart16550"))]
	oro_debug_uart16550::init();
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
//#[collapse_debuginfo(yes)]
macro_rules! dbg {
	($tag:literal, $($arg:tt)*) => {{
		$crate::log(format_args!("{}:I:{}", $tag, format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
//#[collapse_debuginfo(yes)]
macro_rules! dbg_err {
	($tag:literal, $($arg:tt)*) => {{
		$crate::log(format_args!("{}:E:{}", $tag, format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
//#[collapse_debuginfo(yes)]
macro_rules! dbg_warn {
	($tag:literal, $($arg:tt)*) => {{
		$crate::log(format_args!("{}:W:{}", $tag, format_args!($($arg)*)));
	}};
}
