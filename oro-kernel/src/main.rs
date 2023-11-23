#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(never_type)]

extern crate alloc;

mod arch;
mod log;

#[inline(never)]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	self::log::kernel_panic!("{:?}", info);
	self::arch::halt()
}

#[cfg(oro_test)]
type StartReturn = ();
#[cfg(not(oro_test))]
type StartReturn = !;

/// # Safety
/// Do not call directly; only meant to be called by the various bootloaders!
#[inline(never)]
#[no_mangle]
pub unsafe fn _start() -> StartReturn {
	self::arch::init();
	self::log::ok!("boot::arch");

	self::log::ok!("boot");

	#[cfg(not(oro_test))]
	{
		self::log::warning!("boot::kernel will halt");
		self::arch::halt()
	}

	#[cfg(oro_test)]
	{
		self::log::debug!("boot::kernel will return");
	}
}
