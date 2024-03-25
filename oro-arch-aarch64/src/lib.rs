//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(missing_docs)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]
#![cfg_attr(feature = "unstable", feature(const_trait_impl))]

pub(crate) mod arch;
pub(crate) mod mem;

pub use self::{
	arch::Aarch64,
	mem::paging::{
		L0PageTableDescriptor, L1PageTableBlockDescriptor, L1PageTableDescriptor,
		L2PageTableBlockDescriptor, L2PageTableDescriptor, L3PageTableBlockDescriptor, PageTable,
		PageTableEntry, PageTableEntryAddress, PageTableEntryNextLevelAttr, PageTableEntrySubtype,
		PageTableEntryTableAccessPerm, PageTableEntryType, PageTableEntryValidAttr,
	},
};

#[cfg(feature = "unstable")]
pub use self::mem::paging::{
	PageTableEntryAddressConst, PageTableEntryNextLevelAttrConst, PageTableEntryValidAttrConst,
};
