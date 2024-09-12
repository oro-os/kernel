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
pub mod sync;

pub(crate) mod init;

use oro_elf::{ElfClass, ElfEndianness, ElfMachine};
use oro_mem::{pfa::filo::FiloPageFrameAllocator, translate::OffsetTranslator};

/// The ELF class for the AArch64 architecture.
pub const ELF_CLASS: ElfClass = ElfClass::Class64;
/// The ELF endianness for the AArch64 architecture.
///
/// Currently only little-endian is supported.
pub const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
/// The ELF machine type for the AArch64 architecture.
pub const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

/// Type alias for the PFA (page frame allocator) implementation used
/// by the architecture.
pub(crate) type Pfa = FiloPageFrameAllocator<OffsetTranslator>;

/// Type alias for the Oro kernel core-local instance type.
pub(crate) type Kernel = oro_kernel::Kernel<
	CoreState,
	Pfa,
	OffsetTranslator,
	crate::mem::address_space::AddressSpaceLayout,
	crate::sync::InterruptController,
>;

/// Architecture-specific core-local state.
pub(crate) struct CoreState {
	// XXX DEBUG
	#[doc(hidden)]
	#[expect(dead_code)]
	pub unused: u64,
}
