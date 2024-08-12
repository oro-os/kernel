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
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(internal_features, clippy::module_name_repetitions)]
#![feature(core_intrinsics, naked_functions)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]

#[cfg(debug_assertions)]
pub(crate) mod dbgutil;

pub(crate) mod arch;
pub(crate) mod asm;
pub(crate) mod mair;
pub(crate) mod mem;
pub(crate) mod reg;
pub(crate) mod xfer;

pub use self::arch::{
	init_kernel_primary, init_kernel_secondary, init_preboot_primary, Aarch64, Config,
};
