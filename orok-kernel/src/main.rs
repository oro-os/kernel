#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]
#![cfg_attr(not(test), no_main)]

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
	// SAFETY: We're aware we're about to halt.
	unsafe {
		orok_arch::halt();
	}
}

/// Main entry point for the Oro kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It is called
/// by the Limine bootloader.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
	// SAFETY: This is temporary.
	unsafe {
		orok_arch::halt();
	}
}

#[cfg(test)]
fn main() {
	panic!("Don't run this directly.");
}
