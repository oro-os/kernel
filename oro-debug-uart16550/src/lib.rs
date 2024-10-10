//! Early-stage debugging support for the 16550 UART on x86_64
#![cfg_attr(not(test), no_std)]
#![cfg(target_arch = "x86_64")]

use core::fmt::{self, Write};

use spin::mutex::fair::FairMutex;
use uart_16550::SerialPort;

/// The shared serial port for the system.
static SERIAL: FairMutex<SerialPort> = FairMutex::new(unsafe { SerialPort::new(0x3F8) });

/// Initializes the UART.
pub fn init() {
	SERIAL.lock().init();
}

/// Logs a message to the UART.
#[expect(clippy::missing_panics_doc)]
pub fn log(message: fmt::Arguments) {
	writeln!(SERIAL.lock(), "{message}").unwrap();
}
