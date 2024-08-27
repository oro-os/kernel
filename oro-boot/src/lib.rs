//! Boot routine for the Oro kernel.
//!
//! This crate is intended to be used by bootloaders
//! to boot into the Oro kernel via a standardized
//! and safe interface.
//!
//! The role of a bootloader implementation in Oro is to ultimately
//! call this function with a proper configuration, which
//! provides a clean and standardized way of initializing and booting
//! into the Oro kernel without needing to know the specifics of the
//! kernel's initialization process.
//!
//! There are a _lot_ of safety requirements for running the initialization
//! sequence; please read _and understand_ the documentation for the
//! [`boot_to_kernel`] function before calling it.
#![cfg_attr(not(test), no_std)]
#![feature(more_qualified_paths)]

mod init;

// Re-export common stuff that will be considered stable
// for pre-boot environments to use.
pub use self::init::boot_to_kernel;
pub use oro_arch::Target;
pub use oro_common::{
	arch::Arch,
	dbg, dbg_err, dbg_warn,
	mem::{
		region::{MemoryRegion, MemoryRegionType},
		translate::{OffsetPhysicalAddressTranslator, PhysicalAddressTranslator},
	},
	preboot::{ModuleDef, PrebootConfig, PrebootPrimaryConfig},
};
