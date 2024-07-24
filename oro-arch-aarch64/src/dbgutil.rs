//! Provides AArch64-specific dbgutil stubs and facilities.
//!
//! For more information, see the `dbgutil` module in `oro-common`.

#[cfg(not(debug_assertions))]
compile_error!("The `dbgutil` module should only be used in debug builds.");

use core::arch::asm;

/// A function that does nothing but has at least
/// one instruction in it. Debuggers should set a
/// breakpoint on this function to stop execution
/// at the end of a dbg util function to get the result.
#[link_section = ".text.force_keep"]
#[no_mangle]
#[naked]
pub extern "C" fn __oro_dbgutil_finish() {
	// A single nop
	unsafe {
		asm!("nop", options(noreturn));
	}
}

/// Performs a translation as though it were EL1 with
/// read permissions. The result is stored in
/// `PAR_EL1`.
///
/// Pass the virtual address to translate in `x0`.
#[link_section = ".text.force_keep"]
#[no_mangle]
#[naked]
pub extern "C" fn __oro_dbgutil_ATS1E1R() -> ! {
	unsafe {
		asm!("AT S1E1R, x0", "b __oro_dbgutil_finish", options(noreturn));
	}
}
