mod critical;
mod irq;
mod mem;
mod serial;

use ::bootloader::boot_info::MemoryRegions;
use ::x86_64::VirtAddr;

pub use critical::run_critical_section;
pub use serial::{get_serial_logger, SerialLogger};

pub fn init(physical_memory_offset: u64, memory_regions: &'static MemoryRegions) {
	println!("cpu is x86_64");
	irq::init();
	println!("... irq OK");
	mem::init(VirtAddr::new(physical_memory_offset), memory_regions);
	println!("... memory OK");
}

pub fn halt() {
	::x86_64::instructions::hlt();
}
