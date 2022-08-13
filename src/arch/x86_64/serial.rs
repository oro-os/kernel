//! Initializes serial port communications on the x86_64 architecture.

pub use uart_16550::SerialPort as SerialLogger;

/// The default serial port to connect to for basic kernel logging I/O
const SERIAL_IO_PORT: u16 = 0x3F8;

/// Returns the logger instance, if applicable, to be used by the kernel
/// logging mechanism for early-stage logging.
pub fn get_serial_logger() -> Option<SerialLogger> {
	let mut serial_port = unsafe { SerialLogger::new(SERIAL_IO_PORT) };
	serial_port.init();
	Some(serial_port)
}
