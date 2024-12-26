//! Oro kernel dbgutil helpers and stubs.
//!
//! See the `dbgutil` directory in the Oro kernel
//! repository for more information.
#![cfg_attr(not(test), no_std)]
#![cfg(debug_assertions)]
#![feature(naked_functions)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

use core::arch::asm;

#[cfg(debug_assertions)]
use oro_macro::gdb_autoload_inline;

#[cfg(debug_assertions)]
gdb_autoload_inline!("dbgutil.py");

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
#[cfg(any(doc, target_arch = "aarch64"))]
#[link_section = ".text.force_keep"]
#[no_mangle]
#[naked]
pub extern "C" fn __oro_dbgutil_ATS1E1R() -> ! {
	use core::arch::naked_asm;
	unsafe {
		naked_asm!("AT S1E1R, x0", "nop");
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

/// Tells the lock tracker that a lock is about to be acquired.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_lock_acquire(lock_self_addr_do_not_change_this_parameter: usize) {
	unsafe {
		asm!(
			"/*{}*/",
			"nop",
			in(reg) lock_self_addr_do_not_change_this_parameter,
			options(nostack, nomem, preserves_flags)
		);
	}
}

/// Tells the lock tracker that a lock has been released.
///
/// `this` must be the same value as passed to `__oro_dbgutil_lock_acquire`.
#[no_mangle]
#[link_section = ".text.force_keep"]
pub extern "C" fn __oro_dbgutil_lock_release(lock_self_addr_do_not_change_this_parameter: usize) {
	unsafe {
		asm!(
			"/*{}*/",
			"nop",
			in(reg) lock_self_addr_do_not_change_this_parameter,
			options(nostack, nomem, preserves_flags)
		);
	}
}
