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
#![allow(internal_features)]
#![feature(core_intrinsics, never_type)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

pub mod mapper;
pub mod pfa;
pub mod region;
pub mod translate;
