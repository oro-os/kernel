//! High-level runtime support for Oro modules.

#[cfg(feature = "panic_handler")]
#[panic_handler]
#[doc(hidden)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
	// TODO(qix-): Implement panic handler
	loop {
		core::hint::spin_loop();
	}
}

extern "C" {
	fn main();
}

#[doc(hidden)]
#[no_mangle]
pub extern "C" fn _oro_start() -> ! {
	unsafe {
		main();
	}

	// TODO(qix-): Implement exit routine
	loop {
		core::hint::spin_loop();
	}
}
