//! Oro kernel dbgutil helpers and stubs.
//!
//! See the `dbgutil` directory in the Oro kernel
//! repository for more information.
#![cfg_attr(not(test), no_std)]
#![feature(naked_functions)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
#![allow(unused_variables, clippy::inline_always)]

use core::arch::asm;

#[cfg(debug_assertions)]
use oro_macro::gdb_autoload_inline;

#[cfg(debug_assertions)]
gdb_autoload_inline!("dbgutil.py");

/// Performs a translation as though it were EL1 with
/// read permissions. The result is stored in
/// `PAR_EL1`.
///
/// Pass the virtual address to translate in `x0`.
#[cfg(any(doc, target_arch = "aarch64"))]
#[cfg(any(debug_assertions, feature = "force-hooks"))]
#[link_section = ".text.force_keep"]
#[no_mangle]
#[naked]
pub extern "C" fn __oro_dbgutil_ATS1E1R() -> ! {
	use core::arch::naked_asm;
	unsafe {
		naked_asm!("AT S1E1R, x0", "nop");
	}
}

/// Generates GDB tracker service hook functions.
///
/// NOTE: Any changes in the parameter names MUST be reflected in
/// NOTE: the GDB tracker service hook functions in `<repo>/dbgutil`.
macro_rules! hook_functions {
	($($(#[$meta:meta])*$name:ident($($param:ident : $ty:ty),*));* $(;)?) => {
		const _: () = {
			#[used]
			#[no_mangle]
			#[link_section = ".oro_dbgutil.autosym"]
			#[cfg(any(debug_assertions, feature = "force-hooks"))]
			static HOOK_FUNCTIONS: [u8; 0 $(+ 1 + stringify!($name).len())*] = const {
				let mut arr = [0; 0 $(+ 1 + stringify!($name).len())*];
				let mut i = 0;
				$(#[allow(unused_assignments)]
				{
					let bytes = stringify!($name).as_bytes();
					let mut j = 0;
					while j < bytes.len() {
						arr[i] = bytes[j];
						i += 1;
						j += 1;
					}
					arr[i] = 0;
					i += 1
				})*
				arr
			};
		};

		$(
			$(#[$meta])*
			#[no_mangle]
			#[cfg_attr(
				any(debug_assertions, feature = "force-hooks"),
				link_section = ".text.force_keep"
			)]
			#[cfg_attr(not(any(debug_assertions, feature = "force-hooks")), inline(always))]
			#[cfg_attr(any(debug_assertions, feature = "force-hooks"), inline(never))]
			#[allow(clippy::cast_lossless)]
			pub extern "C" fn $name($($param: $ty),*) {
				#[cfg(any(debug_assertions, feature = "force-hooks"))]
				unsafe {
					asm!(
						$(concat!("/*{}", stringify!($param), "*/"), )*
						"nop",
						$(in(reg) $param as u64,)*
						options(nostack, nomem, preserves_flags)
					);
				}
			}
		)*
	}
}

hook_functions! {
	/// Transfer marker stub for `gdbutil` that allows the debugger to switch
	/// to the kernel image at an opportune time.
	__oro_dbgutil_kernel_will_transfer();
	/// Tells dbgutil page frame tracker that a page frame
	/// has been allocated. Assumes a 4KiB page size.
	__oro_dbgutil_pfa_alloc(address: u64);
	/// Tells dbgutil page frame tracker that a page frame
	/// has been freed. Assumes a 4KiB page size.
	__oro_dbgutil_pfa_free(address: u64);
	/// Tells dbgutil page frame tracker that a mass-free event
	/// is about to occur. It will disable the page frame tracker's
	/// `free` breakpoint, if present, to speed up the process.
	///
	/// `__oro_dbgutil_pfa_finished_mass_free` MUST be called
	/// when finished.
	///
	/// If this mass free event is the result of populating
	/// the PFA with initial free pages, set `is_pfa_populating`
	/// to non-zero. Otherwise, set it to `0`.
	///
	/// # Safety
	/// This function is NOT thread-safe. Mass-free events must only
	/// occur when no other threads are running.
	__oro_dbgutil_pfa_will_mass_free(is_pfa_populating: u64);
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
	__oro_dbgutil_pfa_finished_mass_free();
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
	__oro_dbgutil_pfa_mass_free(start_: u64, end_exclusive: u64);
	/// Tells the lock tracker that a lock is about to be acquired.
	__oro_dbgutil_lock_acquire(lock_self: usize);
	/// Tells the lock tracker that a lock has been released.
	///
	/// `this` must be the same value as passed to `__oro_dbgutil_lock_acquire`.
	__oro_dbgutil_lock_release(lock_self: usize);

	/// Tells the core ID tracker that a core ID function was set. The tracker will
	/// then track the ID from this point forward.
	__oro_dbgutil_core_id_fn_was_set(core_id: u32);
	/// Tells the core ID tracker that a core ID was retrieved. The tracker will
	/// validate that the ID returned is the same as the one at time of
	/// [`__oro_dbgutil_core_id_fn_was_set`].
	__oro_dbgutil_core_id_fn_was_called(core_id: u32);
	/// Tells the tab tracker that a page is being allocated during an addition.
	__oro_dbgutil_tab_page_alloc(page: u64, level: usize, index: usize);
	/// Tells the tab tracker that we lost as race condition with allocating a page
	/// and that we're freeing the page back to the system.
	__oro_dbgutil_tab_page_already_allocated(page: u64, level: usize, index: usize);
	/// Tells the tab tracker that we committed a page to the global tab (allocated + one the race).
	__oro_dbgutil_tab_page_commit(page: u64, level: usize, index: usize);
	/// Tells the tab tracker that a tab was added.
	__oro_dbgutil_tab_add(id: u64, ty: u64, slot_addr: usize);
	/// Tells the tab tracker that a tab has a new user.
	__oro_dbgutil_tab_user_add(id: u64, ty: u64, slot_addr: usize);
	/// Tells the tab tracker that a tab has lost a user.
	__oro_dbgutil_tab_user_remove(id: u64, ty: u64, slot_addr: usize);
	/// Tells the tab tracker that a tab was dropped.
	__oro_dbgutil_tab_drop(id: u64, ty: u64, slot_addr: usize);
	/// Tells the tab tracker that a tab was locked for reading.
	__oro_dbgutil_tab_lock_read_acquire(id: u64, ty: u64, slot_addr: usize, count: usize, locked_core: usize, our_core: usize);
	/// Tells the tab tracker that a tab was unlocked for reading.
	__oro_dbgutil_tab_lock_read_release(id: u64, ty: u64, slot_addr: usize, count: usize, locked_core: usize, our_core: usize);
	/// Tells the tab tracker that a tab was locked for writing.
	__oro_dbgutil_tab_lock_write_acquire(id: u64, ty: u64, slot_addr: usize, count: usize, locked_core: usize, our_core: usize);
	/// Tells the tab tracker that a tab was unlocked for writing.
	__oro_dbgutil_tab_lock_write_release(id: u64, ty: u64, slot_addr: usize, count: usize, locked_core: usize, our_core: usize);
}
