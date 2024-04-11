//! x86_64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![feature(const_mut_refs)]
#![cfg(not(all(doc, not(target_arch = "x86_64"))))]
#![cfg_attr(feature = "unstable", feature(core_intrinsics))]
#![cfg_attr(feature = "unstable", allow(internal_features))]

pub mod arch;
pub mod asm;
pub mod mem;

pub use self::{
	arch::X86_64,
	mem::{
		paging::{AvailableFields, PageTable, PageTableEntry},
		pfa::FixedAddressPageFrameManager,
	},
};
