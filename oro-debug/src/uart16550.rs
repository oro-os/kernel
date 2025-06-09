//! Early-stage debugging support for the 16550 UART on x86_64
use core::fmt::{self, Write};

use oro_kernel_sync::{Lock, TicketMutex};
use uart_16550::SerialPort;

/// The shared serial port for the system.
static SERIAL: TicketMutex<SerialPort> = TicketMutex::new(unsafe { SerialPort::new(0x3F8) });

/// Initializes the UART.
pub fn init() {
	SERIAL.lock().init();
}

/// Logs a message to the UART.
pub fn log(message: fmt::Arguments<'_>) {
	writeln!(SERIAL.lock(), "{message}").unwrap();
}

/// Logs a module-level debug line to the UART.
pub fn log_debug_bytes(prefix: &str, line: &[u8]) {
	let mut serial = SERIAL.lock();
	for byte in prefix.bytes() {
		serial.send(byte);
	}
	for byte in line {
		serial.send(*byte);
	}
	serial.send(b'\n');
}

/// Logs a string directly to the UART.
pub fn log_str_raw(string: &str) {
	let mut serial = SERIAL.lock();
	for byte in string.bytes() {
		serial.send(byte);
	}
}
