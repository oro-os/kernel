//! Early-stage debugging support for the 16550 UART on x86_64
#![cfg_attr(not(test), no_std)]
#![cfg(target_arch = "x86_64")]

use core::fmt::{self, Write};
use oro_sync::spinlock::unfair::UnfairSpinlock;
use uart_16550::SerialPort;

/// The shared serial port for the system.
static SERIAL: UnfairSpinlock<SerialPort> = UnfairSpinlock::new(unsafe { SerialPort::new(0x3F8) });

/// Initializes the UART.
pub fn init() {
	// SAFETY(qix-): This is safe, even if we call it multiple times.
	unsafe {
		SERIAL.lock().init();
	}
}

/// Logs a message to the UART.
#[expect(clippy::missing_panics_doc)]
pub fn log(message: fmt::Arguments) {
	// NOTE(qix-): This unsafe block MUST NOT PANIC.
	unsafe { writeln!(SERIAL.lock(), "{message}") }.unwrap();
}
