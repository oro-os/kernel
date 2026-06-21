#![no_std]
#![no_main]

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
	unsafe { ::orok_limine::init() }
}
