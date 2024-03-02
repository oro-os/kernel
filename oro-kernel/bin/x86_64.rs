#![no_std]
#![no_main]

use oro_arch_x86_64::X86_64;
use oro_common::boot::{BootInstanceType, KernelBootConfig};

#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_kernel::panic::<X86_64>(info)
}

/// Main entry point for the Oro kernel. Bootloaders jump
/// to this function to start the kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It should be
/// treated as an ELF entry point and jumped to by the
/// bootloader.
#[inline(never)]
#[cold]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
	static BOOT_CONFIG: KernelBootConfig = KernelBootConfig {
		num_instances: 1,
		memory_regions: &[],
	};

	::oro_kernel::boot::<X86_64>(&BOOT_CONFIG, BootInstanceType::Primary)
}
