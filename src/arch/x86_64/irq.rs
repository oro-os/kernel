use ::lazy_static::lazy_static;
use ::x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
	static ref IDT: InterruptDescriptorTable = {
		let mut idt = InterruptDescriptorTable::new();
		idt.breakpoint.set_handler_fn(irq_breakpoint);
		idt
	};
}

pub fn init() {
	IDT.load();
}

extern "x86-interrupt" fn irq_breakpoint(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: BREAKPOINT --\n{:#?}", stack_frame);
}
