#![no_std]
#![no_main]

use oro_arch_x86_64::X86_64;
use oro_common::Arch;

#[inline(never)]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
	X86_64::halt()
}
