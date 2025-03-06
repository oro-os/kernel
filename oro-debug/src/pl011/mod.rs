//! PL011 UART support used for debugging output on certain architectures/platforms.
//!
//! Note that this is a very primitive implementation, suitable for only what
//! the Oro kernel needs.

mod driver;

use core::fmt::{self, Write};

use oro_sync::{Lock, TicketMutex};

/// The shared serial port for the system.
// NOTE(qix-): This is a temporary solution until pre-boot module loading
// NOTE(qix-): is implemented.
static SERIAL: TicketMutex<Option<driver::PL011>> = TicketMutex::new(None);

/// Initializes the PL011.
pub fn init(offset: usize) {
	// SAFETY(qix-): This is more or less safe, even if called multiple times.
	unsafe {
		// NOTE(qix-): This is set up specifically for QEMU.
		// NOTE(qix-): It is a stop gap measure for early-stage-development
		// NOTE(qix-): debugging and will eventually be replaced with a
		// NOTE(qix-): proper preboot module loader.
		*(SERIAL.lock()) = Some(driver::PL011::new(
			0x900_0000 + offset,
			24_000_000,
			115_200,
			driver::DataBits::Eight,
			driver::StopBits::One,
			driver::Parity::None,
		));
	}
}

/// Logs a message to the PL011.
pub fn log(message: fmt::Arguments<'_>) {
	if let Some(serial) = SERIAL.lock().as_mut() {
		writeln!(serial, "{message}")
	} else {
		Ok(())
	}
	.unwrap();
}

/// Logs a module-level debug line to the PL011.
pub fn log_debug_bytes(prefix: &str, line: &[u8]) {
	if let Some(serial) = SERIAL.lock().as_mut() {
		serial.block_write_all(prefix.as_bytes());
		serial.block_write_all(line);
		serial.block_write_data_byte(b'\n');
	}
}

/// Logs a string directly to the PL011.
pub fn log_str_raw(string: &str) {
	if let Some(serial) = SERIAL.lock().as_mut() {
		serial.block_write_all(string.as_bytes());
	}
}
