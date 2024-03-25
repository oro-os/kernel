//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(missing_docs)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]

pub(crate) mod arch;
pub(crate) mod mem;

pub use self::{
	arch::Aarch64,
	mem::paging::{
		L012PageTableBlock, L012PageTableDescriptor, L3PageTableBlock, PageTable, PageTableEntry,
		PageTableEntryType,
	},
};
