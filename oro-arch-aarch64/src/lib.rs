//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]
#![cfg_attr(feature = "unstable", feature(const_trait_impl))]

pub(crate) mod arch;
pub(crate) mod mem;

pub mod reg;

pub use self::{arch::Aarch64, mem::paging};
