//! Implementation of architecture-specific functionality
//! for the x86_64 class of processors.

mod critical;
mod irq;
mod mem;
mod serial;

use ::bootloader::boot_info::MemoryRegions;
use ::x86_64::VirtAddr;

pub use critical::run_critical_section;
pub use serial::{get_serial_logger, SerialLogger};

/// Initialize the x86_64 CPU.
///
/// # Arguments
///
/// * `physical_memory_offset` - the base address of the linear physical memory map set
///   up by the bootloader
/// * `memory_regions` - the slice of [`bootloader::boot_info::MemoryRegion`]s detected
///   by the BIOS indicating the available, unused regions of physical memory
pub fn init(physical_memory_offset: u64, memory_regions: &'static MemoryRegions) {
	println!("cpu is x86_64");
	irq::init();
	println!("... irq OK");
	mem::init(VirtAddr::new(physical_memory_offset), memory_regions);
	println!("... memory OK");
}

/// Immediately and unconditionally halt the CPU.
///
/// **THIS IS NOT TO BE USED TO SHUT DOWN THE MACHINE.**
pub fn halt() {
	::x86_64::instructions::hlt();
}
