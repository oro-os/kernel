#![no_std]
#![no_main]

mod arch;

use lazy_static::lazy_static;
use spin::Mutex;

// XXX TODO DEBUG
#[cfg(target_arch = "x86_64")]
use uart_16550::SerialPort;

// XXX TODO DEBUG
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

// XXX TODO DEBUG
trait DebugPrint {
	fn dbgprint(self);
}

// XXX TODO DEBUG
impl DebugPrint for &str {
	fn dbgprint(self) {
		use core::fmt::Write;
		let _ = SERIAL.lock().write_str(self);
	}
}

// XXX TODO DEBUG
macro_rules! dbg {
	($($e:expr),*) => {
		$($e.dbgprint();)*
		"\n".dbgprint();
	}
}

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	dbg!("kernel panic"); // XXX TODO DEBUG
	halt()
}

/// # Safety
/// Do not call directly; only meant to be called by the various bootloaders!
#[inline(never)]
#[no_mangle]
pub unsafe fn _start() -> ! {
	self::arch::init();

	dbg!("Oro kernel has booted successfully!"); // XXX TODO DEBUG
	halt()
}
