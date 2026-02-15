//! Main entry point for the Limine bootloader stage
//! of the Oro kernel on the AArch64 architecture.
#![no_std]
#![no_main]

/// Panic handler for the kernel.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo<'_>) -> ! {
	// SAFETY: This is the architecture-specific entry function, the
	// SAFETY: only allowed place to call this function.
	unsafe { ::orok_boot_limine::panic(info) }
}

/// Main entry point for the Limine bootloader stage
/// for the Oro kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It is called
/// by the Limine bootloader.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
	// SAFETY: This is the architecture-specific entry function, the
	// SAFETY: only allowed place to call this function.
	unsafe { ::orok_boot_limine::init() }
}
