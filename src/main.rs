#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}

static HELLO: &[u8] = b"Hello, Oro!";

fn oro_init(boot_info: &'static mut bootloader::BootInfo) -> ! {
	if let Some(_framebuffer) = boot_info.framebuffer.as_mut() {
		for byte in framebuffer.buffer_mut() {
			*byte = 0x90;
		}
	} else {
		let vga_buffer = 0xb8000 as *mut u8;
		for (i, &byte) in HELLO.iter().enumerate() {
			unsafe {
				*vga_buffer.offset(i as isize * 2) = byte;
				*vga_buffer.offset(i as isize * 2 + 1) = 0xb;
			}
		}
	}

	loop {}
}

bootloader::entry_point!(oro_init);
