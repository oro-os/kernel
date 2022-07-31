mod irq;
mod serial;

pub use serial::{get_serial_logger, SerialLogger};

pub fn init() {
	println!("cpu is x86_64");
	irq::init();
	println!("... irq OK");
}

pub fn halt() {
	::x86_64::instructions::hlt();
}
