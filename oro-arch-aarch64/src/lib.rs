//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
//!
//! # Architecture-Specific Requirements for Initialization
//! When initializing the kernel via `oro_boot::boot_to_kernel()`, the following
//! considerations must be made:
//!
//! ### Memory Layout
//! For the most part, the preboot environment's memory layout is left
//! undefined and thus untouched. However on AArch64, the following must be true:
//!
//! - `TCR_EL1.TG0` must be set to 4KiB granule size upon calling `oro_common::init()`.
//! - `TCR_EL1.T0SZ` must encompass enough memory for a identity maps of physical pages.
//!   It is up to the preboot stage to determine an appropriate value, but it is recommended
//!   to set it to 16.
//! - `TTBR0_EL1` must be left undefined or set to 0 and not be relied upon for any execution,
//!   as the initialization subroutine will overwrite it.
//!
//! ### After Transfer Behavior
//! All of TT0 is unmapped and TTBR0 is set to 0. This, however, means nothing to the
//! preboot environment, as the preboot environment MUST NOT rely on TTBR0 for any resource
//! allocation or mapping.
#![no_std]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]
#![expect(internal_features)]
#![feature(core_intrinsics)]
// SAFETY(qix-): It's probably accepted, and I want to refactor the
// SAFETY(qix-): page table implementaiton anyway at some point so
// SAFETY(qix-): this is probably fine for now.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/122034
#![feature(ptr_as_ref_unchecked)]

pub mod asm;
pub mod boot;
pub mod mair;
pub mod mem;
pub mod psci;
pub mod reg;

pub(crate) mod init;

use oro_elf::{ElfClass, ElfEndianness, ElfMachine};

/// The ELF class for the AArch64 architecture.
pub const ELF_CLASS: ElfClass = ElfClass::Class64;
/// The ELF endianness for the AArch64 architecture.
///
/// Currently only little-endian is supported.
pub const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
/// The ELF machine type for the AArch64 architecture.
pub const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

/// Zero-sized type for specifying the architecture-specific types
/// used throughout the `oro-kernel` crate.
pub(crate) struct Arch;

impl oro_kernel::Arch for Arch {
	type AddrSpace = crate::mem::address_space::AddressSpaceLayout;
	type CoreState = ();
	type ThreadState = ();

	fn make_instance_unique(
		_mapper: &<Self::AddrSpace as oro_mem::mapper::AddressSpace>::UserHandle,
	) -> Result<(), oro_mem::mapper::MapError> {
		todo!("make_instance_unique()");
	}

	fn new_thread_state(_stack_ptr: usize, _entry_point: usize) -> Self::ThreadState {
		todo!("new_thread_state()");
	}

	fn initialize_thread_mappings(
		_thread: &<Self::AddrSpace as oro_mem::mapper::AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
	) -> Result<(), oro_mem::mapper::MapError> {
		todo!("initialize_thread_mappings()");
	}

	fn reclaim_thread_mappings(
		_thread: &<Self::AddrSpace as oro_mem::mapper::AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
	) {
		todo!("reclaim_thread_mappings()");
	}
}

/// Type alias for the Oro kernel core-local instance type.
pub(crate) type Kernel = oro_kernel::Kernel<Arch>;

/// Architecture-specific core-local state.
#[expect(dead_code)] // XXX DEBUG
pub(crate) struct CoreState {
	// XXX DEBUG
	#[doc(hidden)]
	pub unused: u64,
}
