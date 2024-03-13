//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
#![no_std]
#![deny(missing_docs)]

pub mod boot;
pub mod lock;

pub(crate) mod arch;
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

/// Utility macro that requires that it's present inside of an
/// unsafe block.
#[macro_export]
macro_rules! assert_unsafe {
	() => {{
		let _ptr = 0 as *const ();
		let _this_macro_must_be_used_in_an_unsafe_context = *_ptr;
	}};
}
