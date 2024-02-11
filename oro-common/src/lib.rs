//! Common code and utilities for crates within
//! the [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel project.
#![no_std]
#![deny(missing_docs)]

mod arch;
mod boot;
mod dbg;

pub use self::{
	arch::Arch,
	boot::{BootConfig, BootInstanceType},
};
