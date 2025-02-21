//! Main entry point for the Oro kernel on the AArch64 architecture.
#![no_std]
#![no_main]

/// Panic handler for the kernel.
#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo<'_>) -> ! {
	oro_debug::dbg_err!("panic: {info:?}");
	oro_arch_aarch64::asm::halt();
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
	// SAFETY: This is the only place it's being called.
	unsafe {
		oro_arch_aarch64::boot::boot_primary();
	}
}
