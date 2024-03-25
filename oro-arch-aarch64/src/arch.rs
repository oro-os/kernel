use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
};
use oro_common::{sync::UnfairSpinlock, Arch};
use oro_serial_pl011 as pl011;

static mut SERIAL: UnfairSpinlock<Aarch64, MaybeUninit<pl011::PL011>> =
	UnfairSpinlock::new(MaybeUninit::uninit());

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

#[allow(clippy::inline_always)]
unsafe impl Arch for Aarch64 {
	type InterruptState = usize;

	unsafe fn init_shared() {
		// TODO(qix-): This is set up specifically for QEMU.
		// TODO(qix-): This will need to be adapted to handle
		// TODO(qix-): different UART types and a configurable
		// TODO(qix-): base address / settings in the future.
		SERIAL.lock().write(pl011::PL011::new::<Self>(
			0x900_0000,
			24_000_000,
			115_200,
			pl011::DataBits::Eight,
			pl011::StopBits::One,
			pl011::Parity::None,
		));
	}

	unsafe fn init_local() {}

	#[cold]
	fn halt() -> ! {
		loop {
			unsafe {
				asm!("wfi");
			}
		}
	}

	#[inline(always)]
	fn disable_interrupts() {
		unsafe {
			asm!("msr daifset, 0xf", options(nostack, nomem, preserves_flags));
		}
	}

	#[inline(always)]
	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("mrs {}, daif", out(reg) flags, options(nostack, nomem));
		}
		flags
	}

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
