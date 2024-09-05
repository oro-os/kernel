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

	let memory::PreparedMemory { pfa: _pfa, pat } = memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(pat.offset());

	oro_debug::dbg!("is this thing on?");

	crate::asm::halt();
}
