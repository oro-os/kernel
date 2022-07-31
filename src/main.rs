#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]

mod gfx;
#[macro_use]
mod logger;
mod arch;
mod oro;

use core::cell::UnsafeCell;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
	println!("\n\n-- ORO PANICKED --\n{}", info);
	halt();
}

pub fn halt() -> ! {
	println!("\n\n-- ORO HALTED --");
	loop {
		arch::halt();
	}
}

fn _start_oro(boot_info: &'static mut bootloader::BootInfo) -> ! {
	if let Some(logger) = arch::get_serial_logger() {
		logger::set_global_serial_logger(logger);
	}

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

		unsafe {
			logger::init_global_framebuffer_logger(
				gfx::PADDING + gfx::LEFT_GUTTER_WIDTH,
				gfx::PADDING,
				fb_info.horizontal_resolution - gfx::PADDING,
				fb_info.vertical_resolution - gfx::PADDING,
				rasterizer,
			)
		};
	}

	oro::init();

	halt();
}

bootloader::entry_point!(_start_oro);
