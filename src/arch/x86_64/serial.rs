pub use uart_16550::SerialPort as SerialLogger;

const SERIAL_IO_PORT: u16 = 0x3F8;

pub fn get_serial_logger() -> Option<SerialLogger> {
	let mut serial_port = unsafe { SerialLogger::new(SERIAL_IO_PORT) };
	serial_port.init();
	Some(serial_port)
}
