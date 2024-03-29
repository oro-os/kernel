//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
#![no_std]
#![deny(missing_docs, clippy::integer_division)]
#![allow(clippy::module_name_repetitions)]
#![feature(const_trait_impl)]

pub mod mem;
pub mod sync;

pub(crate) mod arch;
pub(crate) mod dbg;
pub(crate) mod init;
pub(crate) mod unsafe_macros;

pub use self::{
	arch::Arch,
	init::{boot_to_kernel, PrebootConfig, PrebootPrimaryConfig},
};
