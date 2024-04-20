//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(internal_features)]
#![feature(const_trait_impl, core_intrinsics)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]

pub(crate) mod arch;
pub(crate) mod mem;
pub(crate) mod reg;

pub use self::arch::Aarch64;
