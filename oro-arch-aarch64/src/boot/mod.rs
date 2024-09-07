//! Boot routine for the AArch64 architecture.
//!
//! This module prepares the kernel on AArch64
//! directly after being transferred to by the
//! bootloader.

mod memory;
mod protocol;

use oro_boot_protocol::device_tree::{DeviceTreeDataV0, DeviceTreeKind};
use oro_debug::dbg;
use oro_dtb::FdtHeader;
use oro_mem::translate::Translator;

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

	#[allow(unused_variables)] // XXX(qix-): Temporary for CI
	let memory::PreparedMemory { pfa: _pfa, pat } = memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(pat.offset());

	// Get the devicetree blob.
	let DeviceTreeKind::V0(dtb) = protocol::DTB_REQUEST
		.response()
		.expect("no DeviceTree blob response was provided")
	else {
		panic!("DeviceTree blob response was provided but was the wrong revision");
	};

	let DeviceTreeDataV0 { base, length } = dtb.assume_init_ref();

	dbg!("got DeviceTree blob of {} bytes", length);

	let dtb = FdtHeader::from(pat.translate::<u8>(*base), Some(*length)).expect("dtb is invalid");
	dbg!("dtb is valid; primary core id is {}", dtb.phys_id());

	for tkn in dtb.iter() {
		dbg!("@ {tkn:?}");
	}

	crate::asm::halt();
}
