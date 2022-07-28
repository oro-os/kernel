#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]

mod gfx;
#[macro_use]
mod logger;
mod init;

use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use logger::FrameBufferLogger;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	println!();
	println!();
	println!("-- ORO PANICKED --\n{}", info);
	loop {}
}

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
		rasterizer.set_fg(0xFF, 0xFF, 0xFF, 0xFF);
		rasterizer.set_accent(0x78, 0x00, 0xFF, 0x80);
		rasterizer.clear_screen();
		rasterizer.draw_boot_frame();

		let logger = FrameBufferLogger::new(
			gfx::PADDING + gfx::LEFT_GUTTER_WIDTH,
			gfx::PADDING,
			fb_info.horizontal_resolution - gfx::PADDING,
			fb_info.vertical_resolution - gfx::PADDING,
			rasterizer,
		);

		logger::set_global_framebuffer_logger(logger);
	}

	init::init();

	println!();
	println!();
	println!("-- ORO HALTED --");
	loop {}
}

bootloader::entry_point!(oro_init);
