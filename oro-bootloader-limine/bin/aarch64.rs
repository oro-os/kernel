#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;
use oro_common::Arch;

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	Aarch64::halt()
}
