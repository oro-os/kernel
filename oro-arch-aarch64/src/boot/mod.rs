//! Boot routine for the AArch64 architecture.
//!
//! This module prepares the kernel on AArch64
//! directly after being transferred to by the
//! bootloader.

mod memory;
mod protocol;
mod secondary;

use oro_debug::dbg;
#[cfg(debug_assertions)]
use oro_mem::phys::{Phys, PhysAddr};

/// The number of pages to allocate for the secondary core stacks.
// TODO(qix-): Make this configurable.
const SECONDARY_STACK_PAGES: usize = 16;

/// Boots the primary core on AArch64.
///
/// # Safety
/// Meant only to be called by the entry point.
/// Do not call this directly. It does not reset
/// the kernel or anything else magic like that.
///
/// # Panics
/// Panics if the DeviceTree blob is not provided.
pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();

	memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(Phys::from_address_unchecked(0).virt());

	// Initialize the primary core.
	crate::init::initialize_primary();

	{
		// Boot secondaries.
		let num_cores = secondary::boot_secondaries(SECONDARY_STACK_PAGES);
		dbg!("continuing with {num_cores} cores");
	}

	crate::init::boot();
}
