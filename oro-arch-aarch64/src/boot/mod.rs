mod memory;
mod protocol;

pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();

	panic!("ready");
}
