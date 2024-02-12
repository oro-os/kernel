//! aarch64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
#![no_std]
#![deny(missing_docs)]
#![cfg(not(all(doc, not(target_arch = "aarch64"))))]

use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
};
use oro_common::Arch;
use oro_serial_pl011::PL011;
use spin::Mutex;

static mut SERIAL: MaybeUninit<Mutex<self::PL011>> = MaybeUninit::uninit();

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

impl Arch for Aarch64 {
	unsafe fn init_shared() {
		// TODO(qix-): This is set up specifically for QEMU.
		// TODO(qix-): This will need to be adapted to handle
		// TODO(qix-): different UART types and a configurable
		// TODO(qix-): base address / settings in the future.
		SERIAL.write(Mutex::new(PL011::new(0x9000000, 24000000, 115200, 8, 1)));
	}

	unsafe fn init_local() {}

	fn halt() -> ! {
		loop {
			unsafe {
				asm!("wfi");
			}
		}
	}

	fn log(message: fmt::Arguments) {
		unsafe {
			writeln!(SERIAL.assume_init_ref().lock(), "{message}").unwrap();
		}
	}
}
