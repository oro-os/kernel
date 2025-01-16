//! Early-stage debugging support for the 16550 UART on x86_64
#![cfg_attr(not(test), no_std)]
#![cfg(any(doc, target_arch = "x86_64"))]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

use core::fmt::{self, Write};

use oro_sync::{Lock, TicketMutex};
use uart_16550::SerialPort;

/// The shared serial port for the system.
static SERIAL: TicketMutex<SerialPort> = TicketMutex::new(unsafe { SerialPort::new(0x3F8) });

/// Initializes the UART.
pub fn init() {
	SERIAL.lock().init();
}

/// Logs a message to the UART.
pub fn log(message: fmt::Arguments) {
	writeln!(SERIAL.lock(), "{message}").unwrap();
}

/// Logs a module-level debug line to the PL011.
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
