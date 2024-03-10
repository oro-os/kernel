//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
#![no_std]
#![deny(missing_docs)]

pub(crate) mod arch;
pub mod boot;
pub(crate) mod dbg;
pub(crate) mod init;
pub(crate) mod mem;

pub use self::{
	arch::Arch,
	init::boot_to_kernel,
	mem::{
		pfa::PageFrameAllocator,
		pfa_filo::{FiloPageFrameAllocator, FiloPageFrameManager},
		region::{MemoryRegion, MemoryRegionType},
	},
};
