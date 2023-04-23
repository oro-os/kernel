#![no_std]
#![no_main]

use lazy_static::lazy_static;
use spin::Mutex;
#[cfg(target_arch = "x86_64")]
use uart_16550::SerialPort;

lazy_static! {
	#[cfg(target_arch = "x86_64")]
	static ref SERIAL: Mutex<SerialPort> = {
		let mut serial_port = unsafe { SerialPort::new(0x3F8) };
		serial_port.init();
		Mutex::new(serial_port)
	};
}

#[cfg(target_arch = "x86_64")]
unsafe fn halt() -> ! {
	use core::arch::asm;
	asm!("cli");
	loop {
		asm!("hlt");
	}
}

trait DebugPrint {
	fn dbgprint(self);
}

impl DebugPrint for &str {
	fn dbgprint(self) {
		use core::fmt::Write;
		let _ = SERIAL.lock().write_str(self);
	}
}

macro_rules! dbg {
	($($e:expr),*) => {
		$($e.dbgprint();)*
		"\n".dbgprint();
	}
}

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	dbg!("kernel panic");
	halt()
}

/// # Safety
/// Do not call directly; only meant to be called by the various bootloaders!
#[inline(never)]
#[no_mangle]
pub unsafe fn _start() -> ! {
	dbg!("Oro kernel has booted successfully!");
	halt()
}
