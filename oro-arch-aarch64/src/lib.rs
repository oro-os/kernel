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
use oro_common::{lock::UnfairSpinlock, Arch};
use oro_serial_pl011::PL011;

static mut SERIAL: UnfairSpinlock<Aarch64, MaybeUninit<PL011>> =
	UnfairSpinlock::new(MaybeUninit::uninit());

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

unsafe impl Arch for Aarch64 {
	type InterruptState = usize;

	unsafe fn init_shared() {
		// TODO(qix-): This is set up specifically for QEMU.
		// TODO(qix-): This will need to be adapted to handle
		// TODO(qix-): different UART types and a configurable
		// TODO(qix-): base address / settings in the future.
		SERIAL
			.lock()
			.write(PL011::new(0x900_0000, 24_000_000, 115_200, 8, 1));
	}

	unsafe fn init_local() {}

	fn halt() -> ! {
		loop {
			unsafe {
				asm!("wfi");
			}
		}
	}

	#[allow(clippy::inline_always)]
	#[inline(always)]
	fn disable_interrupts() {
		unsafe {
			asm!("msr daifset, 0xf", options(nostack, nomem, preserves_flags));
		}
	}

	#[allow(clippy::inline_always)]
	#[inline(always)]
	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("mrs {}, daif", out(reg) flags, options(nostack, nomem));
		}
		flags
	}

	#[allow(clippy::inline_always)]
	#[inline(always)]
	fn restore_interrupts(state: Self::InterruptState) {
		unsafe {
			asm!("msr daif, {}", in(reg) state, options(nostack, nomem));
		}
	}

	fn log(message: fmt::Arguments) {
		// NOTE(qix-): This unsafe block MUST NOT PANIC.
		unsafe {
			let mut lock = SERIAL.lock();
			writeln!(lock.assume_init_mut(), "{message}")
		}
		.unwrap();
	}
}
