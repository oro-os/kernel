#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]

mod gfx;
mod logger;

use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use logger::BootLogger;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}

// XXX DEBUG
const FUN_LINES: &'static [&str] = &[
	"initializing memory segment @ 0x000FF000000...",
	"created boot sequence",
	"scanning regions of nonsense... OK",
	"bringing base modules online.... OK",
	"booting system.... 0%",
	"booting system.... 22%",
	"booting system.... 39%",
	"booting system.... 58%",
	"booting system.... 83%",
	"booting system.... 100%",
	"setting system clock... OK (from NTP server)",
	"connecting to base WiFi antenna... OK",
	"leasing DHCP information... OK",
	"florping sixteen gabfloobers... OK (successfully flooped)",
	"system was booted in a mode that will underperform at any task!",
];

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

		let logger = BootLogger::new(
			gfx::PADDING + gfx::LEFT_GUTTER_WIDTH,
			gfx::PADDING,
			fb_info.horizontal_resolution - gfx::PADDING,
			fb_info.vertical_resolution - gfx::PADDING,
			rasterizer,
		);

		logger::set_global_logger(logger);

		// XXX DEBUG
		FUN_LINES
			.iter()
			.cycle()
			.take(500)
			.for_each(|&line| println!("{}", line));
	}

	loop {}
}

bootloader::entry_point!(oro_init);
