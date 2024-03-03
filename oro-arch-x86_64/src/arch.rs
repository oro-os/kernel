use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
};
use oro_common::Arch;
use spin::Mutex;
use uart_16550::SerialPort;

static mut SERIAL: MaybeUninit<Mutex<SerialPort>> = MaybeUninit::uninit();

/// `x86_64` architecture support implementation for the Oro kernel.
pub struct X86_64;

impl Arch for X86_64 {
	unsafe fn init_shared() {
		// Initialize the serial port
		SERIAL.write(Mutex::new(SerialPort::new(0x3F8)));
	}

	unsafe fn init_local() {}

	fn halt() -> ! {
		loop {
			unsafe {
				asm!("cli", "hlt");
			}
		}
	}

	fn disable_interrupts() {
		unsafe {
			asm!("cli", options(nostack, preserves_flags));
		}
	}

	fn enable_interrupts() {
		unsafe {
			asm!("sti", options(nostack, preserves_flags));
		}
	}

	fn log(message: fmt::Arguments) {
		unsafe {
			writeln!(SERIAL.assume_init_ref().lock(), "{message}").unwrap();
		}
	}
}
