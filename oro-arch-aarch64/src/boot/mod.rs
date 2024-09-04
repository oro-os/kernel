mod memory;
mod protocol;

pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();

	let memory::PreparedMemory { mut pfa, pat } = memory::prepare_memory();

	panic!("ready");
}
