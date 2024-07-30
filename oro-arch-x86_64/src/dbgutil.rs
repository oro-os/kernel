//! Stubs and other internal functionality for the `dbgutil` suite
//! of Oro-specific GDB debugging utilities.

#[cfg(not(debug_assertions))]
compile_error!("The `dbgutil` module should only be used in debug builds.");

use core::arch::asm;

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
