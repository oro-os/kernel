//! `x86_64` architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(missing_docs)]
#![cfg(not(all(doc, not(target_arch = "x86_64"))))]

pub(crate) mod arch;
pub(crate) mod mem;

pub use self::{
	arch::X86_64,
	mem::{
		paging::{AvailableFields, PageTable, PageTableEntry},
		pfa::FixedAddressPageFrameManager,
	},
};
