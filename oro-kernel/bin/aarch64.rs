#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;

#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_kernel::panic::<Aarch64>(info)
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
pub unsafe fn _start() -> ! {
	// TODO(qix-): Temporary.
	static BOOT_CONFIG: ::oro_common::BootConfig = ::oro_common::BootConfig {
		instance_type: ::oro_common::BootInstanceType::Primary,
		num_instances: 1,
	};

	::oro_kernel::boot::<Aarch64>(&BOOT_CONFIG)
}
