#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]

mod gfx;

use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use core::ptr::null_mut;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}

static mut GLOBAL_RASTERIZER: *mut gfx::Rasterizer = null_mut();

fn oro_init(boot_info: &'static mut bootloader::BootInfo) -> ! {
	if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
		let fb_info = framebuffer.info();

		use bootloader::boot_info::PixelFormat as BootPixFmt;

		let info = gfx::RasterizerInfo {
			format: match fb_info.pixel_format {
				BootPixFmt::RGB => gfx::PixelFormat::RGB8,
				BootPixFmt::BGR => gfx::PixelFormat::BGR8,
				BootPixFmt::U8 => gfx::PixelFormat::GREY8,
				_ => gfx::PixelFormat::FALLBACK,
			},
			width: fb_info.horizontal_resolution,
			height: fb_info.vertical_resolution,
			pixel_stride: fb_info.bytes_per_pixel,
			stride: fb_info.stride,
		};

		let mut rasterizer = gfx::Rasterizer::new(UnsafeCell::from(framebuffer.buffer_mut()), info);
		rasterizer.set_bg(0, 0, 0, 0);
		rasterizer.set_fg(0x50, 0, 0, 0x70);
		rasterizer.clear();
		rasterizer.draw_boot_frame();
		rasterizer.set_fg(0xFF, 0xFF, 0xFF, 0xFF);

		// XXX DEBUG
		let mut x: usize = 10;
		for b in "Hello, Oro! test::START() 0.394182752256023 <3 {OK} invalid=\x1b".bytes() {
			if (b as char) != ' ' {
				rasterizer.draw_char(x, 10, b);
			}
			x += gfx::GLYPH_WIDTH;
		}

		unsafe {
			GLOBAL_RASTERIZER = &mut rasterizer;
		};
	}

	loop {}
}

bootloader::entry_point!(oro_init);
