//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(missing_docs)]

use core::arch::asm;
use oro_common::Arch;

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

impl Arch for Aarch64 {
	unsafe fn init() {}

	fn halt() -> ! {
		loop {
			unsafe {
				asm!("wfi");
			}
		}
	}
}
