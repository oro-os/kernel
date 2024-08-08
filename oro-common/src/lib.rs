//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
//!
//! # Bootloaders
//! **Do should not use this crate directly.**
//!
//! If you are implementing a bootloader and want to boot into
//! the Oro kernel, see the `oro_boot` crate. If something is
//! missing from that crate that you need to implement a bootloader,
//! please file an issue.
//!
//! # Architectures
//! If you are implementing an architecture for Oro, see the
//! [`crate::arch::Arch`] trait.
#![cfg_attr(not(test), no_std)]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(
	clippy::module_name_repetitions,
	clippy::missing_errors_doc,
	internal_features,
	rustdoc::private_doc_tests
)]
#![feature(core_intrinsics, debug_closure_helpers)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

#[cfg(any(debug_assertions, feature = "dbgutil"))]
mod dbgutil;

pub mod arch;
pub mod boot;
pub mod dbg;
pub mod elf;
pub mod mem;
pub mod preboot;
pub mod proc;
pub mod ser2mem;
pub mod sync;

#[cfg(feature = "kernel")]
pub mod util;
#[cfg(not(feature = "kernel"))]
pub(crate) mod util;
