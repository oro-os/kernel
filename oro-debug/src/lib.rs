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
#![feature(naked_functions)]
#![cfg_attr(not(test), no_std)]

#[cfg(not(debug_assertions))]
compile_error!("The `oro-debug` crate should only be used in debug builds.");

use core::arch::asm;
#[cfg(feature = "dbgutil")]
use oro_common_proc::gdb_autoload_inline;

#[cfg(feature = "dbgutil")]
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
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:I:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
//#[collapse_debuginfo(yes)]
macro_rules! dbg_err {
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:E:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
//#[collapse_debuginfo(yes)]
macro_rules! dbg_warn {
	($($arg:tt)*) => {{
		$crate::log(format_args!("{}:{}:W:{}", ::core::file!(), ::core::line!(), format_args!($($arg)*)));
	}};
}

/// Transfer marker stub for `gdbutil` that allows the debugger to switch
/// to the kernel image at an opportune time.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_kernel_will_transfer() {
	// SAFETY(qix-): This is a marker function for GDB to switch to the kernel image.
	unsafe {
		asm!("nop", options(nostack, nomem, preserves_flags));
	}
}

/// Performs a translation as though it were EL1 with
/// read permissions. The result is stored in
/// `PAR_EL1`.
///
/// Pass the virtual address to translate in `x0`.
#[cfg(target_arch = "aarch64")]
#[link_section = ".text.force_keep"]
#[no_mangle]
#[naked]
pub extern "C" fn __oro_dbgutil_ATS1E1R() -> ! {
	unsafe {
		asm!("AT S1E1R, x0", "nop", options(noreturn));
	}
}

/// Tells dbgutil page frame tracker that a page frame
/// has been allocated. Assumes a 4KiB page size.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_pfa_alloc(address_do_not_change_this_parameter_name: u64) {
	unsafe {
		asm!(
			"/*{}*/",
			"nop",
			in(reg) address_do_not_change_this_parameter_name,
			options(nostack, nomem, preserves_flags)
		);
	}
}

/// Tells dbgutil page frame tracker that a page frame
/// has been freed. Assumes a 4KiB page size.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_pfa_free(address_do_not_change_this_parameter_name: u64) {
	unsafe {
		asm!(
			"/*{}*/",
			"nop",
			in(reg) address_do_not_change_this_parameter_name,
			options(nostack, nomem, preserves_flags)
		);
	}
}

/// Tells dbgutil page frame tracker that a mass-free event
/// is about to occur. It will disable the page frame tracker's
/// `free` breakpoint, if present, to speed up the process.
///
/// `__oro_dbgutil_pfa_finished_mass_free` MUST be called
/// when finished.
///
/// If this mass free event is the result of populating
/// the PFA with initial free pages, set `is_pfa_populating_do_not_change_this_parameter`
/// to non-zero. Otherwise, set it to `0`.
///
/// # Safety
/// This function is NOT thread-safe. Mass-free events must only
/// occur when no other threads are running.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub unsafe extern "C" fn __oro_dbgutil_pfa_will_mass_free(
	is_pfa_populating_do_not_change_this_parameter: u64,
) {
	unsafe {
		asm!(
			"/*{}*/",
			"nop",
			in(reg) is_pfa_populating_do_not_change_this_parameter,
			options(nostack, nomem, preserves_flags)
		);
	}
}

/// Tells dbgutil page frame tracker that a mass-free event
/// just finished. It will re-enable the page frame tracker's
/// `free` breakpoint, if present.
///
/// `__oro_dbgutil_pfa_finished_mass_free` MUST be called
/// when finished.
///
/// # Safety
/// This function is NOT thread-safe. Mass-free events must only
/// occur when no other threads are running.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub unsafe extern "C" fn __oro_dbgutil_pfa_finished_mass_free() {
	unsafe {
		asm!("nop", options(nostack, nomem, preserves_flags));
	}
}

/// Tells the PFA tracker that a region of memory is now free.
///
/// This is a much more efficient way to free memory than
/// calling `__oro_dbgutil_pfa_free` multiple times, and can be
/// used to free large regions of memory at once in lieu of
/// that function.
///
/// The `will_free`/`finished_free` hints do not need to be used
/// unless the `free` breakpoint would otherwise be hit, as it
/// will cause the PFA tracker to warn on a double-free.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_pfa_mass_free(
	start_do_not_change_this_parameter: u64,
	end_exclusive_do_not_change_this_parameter: u64,
) {
	unsafe {
		asm!(
			"/*{} {}*/",
			"nop",
			in(reg) start_do_not_change_this_parameter,
			in(reg) end_exclusive_do_not_change_this_parameter,
			options(nostack, nomem, preserves_flags)
		);
	}
}
