//! Implements the panic handler.

#[panic_handler]
#[doc(hidden)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
	// TODO(qix-): Send panic information somewhere.
	#[cfg(feature = "panic_debug_out_v0")]
	crate::debug_out_v0_println!("panic: {:?}", _info);

	unsafe { super::terminate() }
}
