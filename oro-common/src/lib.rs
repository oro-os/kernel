//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
//!
//! # Bootloaders
//! If you are implementing a bootloader and want to boot into
//! the Oro kernel, see the [`boot_to_kernel`] function.
//!
//! # Architectures
//! If you are implementing an architecture for Oro, see the
//! [`Arch`] trait.
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
#![feature(const_trait_impl, core_intrinsics, debug_closure_helpers)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

#[cfg(debug_assertions)]
mod dbgutil;

pub mod elf;
pub mod mem;
pub mod proc;
pub mod sync;

pub(crate) mod arch;
pub(crate) mod dbg;
pub(crate) mod init;
pub(crate) mod util;

pub use self::{
	arch::Arch,
	init::{boot_to_kernel, ModuleDef, PrebootConfig, PrebootPrimaryConfig},
};
