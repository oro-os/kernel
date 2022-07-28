use crate::logger::SerialLogger;

pub fn get_serial_logger() -> SerialLogger {
	if cfg!(target_arch = "x86_64") {
		const SERIAL_IO_PORT: u16 = 0x3F8;

		let mut serial_port = unsafe { uart_16550::SerialPort::new(SERIAL_IO_PORT) };
		serial_port.init();

		return SerialLogger::IO(serial_port);
	} else if cfg!(target_arch = "aarch64") {
		const SERIAL_PORT_BASE_ADDRESS: usize = 0x1000_0000;

		let mut serial_port = unsafe { uart_16550::MmioSerialPort::new(SERIAL_PORT_BASE_ADDRESS) };
		serial_port.init();

		return SerialLogger::Map(serial_port);
	} else {
		return SerialLogger::None;
	}
}

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;
