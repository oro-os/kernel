//! Boot routine for the AArch64 architecture.
//!
//! This module prepares the kernel on AArch64
//! directly after being transferred to by the
//! bootloader.

mod memory;
mod protocol;

/// Boots the primary core on AArch64.
///
/// # Safety
/// Meant only to be called by the entry point.
/// Do not call this directly. It does not reset
/// the kernel or anything else magic like that.
pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();

	let memory::PreparedMemory {
		pfa: _pfa,
		pat: _pat,
	} = memory::prepare_memory();

	crate::asm::halt();
}
