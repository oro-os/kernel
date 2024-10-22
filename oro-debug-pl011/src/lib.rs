//! PL011 UART support used for debugging output on certain architectures/platforms.
//!
//! Note that this is a very primitive implementation, suitable for only what
//! the Oro kernel needs.
#![no_std]
#![expect(internal_features)]
#![feature(core_intrinsics)]

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
#[expect(clippy::missing_panics_doc)]
pub fn log(message: fmt::Arguments) {
	if let Some(serial) = SERIAL.lock().as_mut() {
		writeln!(serial, "{message}")
	} else {
		Ok(())
	}
	.unwrap();
}
