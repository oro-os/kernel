#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;
use oro_common::Arch;

#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	Aarch64::halt()
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
	::oro_kernel::init::<Aarch64>()
}
