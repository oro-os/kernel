#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;

#[inline(never)]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_bootloader_limine::panic::<Aarch64>(info)
}

/// Main entry point for the Limine bootloader stage
/// for the Oro kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It is called
/// by the Limine bootloader.
#[inline(never)]
#[cold]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
	::oro_bootloader_limine::init::<Aarch64>()
}
