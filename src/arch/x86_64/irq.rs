//! Initializers for the Interrupt Descriptor Table (IDT)
//! for the x86_64 architecture.

use ::lazy_static::lazy_static;
use ::x86_64::structures::idt::{
	InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode,
};

lazy_static! {
	/// The x86_64 Interrupt Descriptor Table (IDT) to be registered
	/// in the CPU.
	static ref IDT: InterruptDescriptorTable = {
		let mut idt = InterruptDescriptorTable::new();
		idt.breakpoint.set_handler_fn(irq_breakpoint);
		idt.invalid_opcode.set_handler_fn(irq_invalid_opcode);
		idt.divide_error.set_handler_fn(irq_div);
		idt.overflow.set_handler_fn(irq_overflow);
		idt.double_fault.set_handler_fn(irq_double_fault);
		idt.page_fault.set_handler_fn(irq_page_fault);
		idt.invalid_tss.set_handler_fn(irq_invalid_tss);
		idt.segment_not_present.set_handler_fn(irq_missing_segment);
		idt.stack_segment_fault.set_handler_fn(irq_stack_fault);
		idt.general_protection_fault.set_handler_fn(irq_gpf);
		idt.debug.set_handler_fn(irq_debug);
		idt.non_maskable_interrupt.set_handler_fn(irq_nmi);
		idt.bound_range_exceeded.set_handler_fn(irq_oob);
		idt.device_not_available
			.set_handler_fn(irq_device_not_available);
		idt.x87_floating_point.set_handler_fn(irq_x87);
		idt.alignment_check.set_handler_fn(irq_chk_alignment);
		idt.machine_check.set_handler_fn(irq_chk_machine);
		idt.simd_floating_point.set_handler_fn(irq_simd_fp);
		idt.virtualization.set_handler_fn(irq_virtualization);
		idt.vmm_communication_exception.set_handler_fn(irq_vmm);
		idt.security_exception.set_handler_fn(irq_security);
		idt
	};
}

/// Loads the IDT into the appropriate CPU register.
pub fn init() {
	IDT.load();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_breakpoint(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: BREAKPOINT --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_debug(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: DEBUG --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_nmi(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: NON-MASkABLE INTERRUPT --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_invalid_opcode(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: INVALID OPCODE --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_oob(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: OUT OF BOUNDS --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_x87(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: x87 FP ERROR --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_simd_fp(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: SIMGD FP ERROR --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_div(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: DIVISION ERROR --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_device_not_available(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: DEVICE NOT AVAILABLE --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_overflow(stack_frame: InterruptStackFrame) {
	println!("\n\n-- ORO EXCEPTION: OVERFLOW --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_double_fault(stack_frame: InterruptStackFrame, error_code: u64) -> ! {
	println!(
		"\n\n-- ORO EXCEPTION: DOUBLE FAULT --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_page_fault(
	stack_frame: InterruptStackFrame,
	fault: PageFaultErrorCode,
) {
	println!(
		"\n\n-- ORO EXCEPTION: PAGE FAULT --\n{:#?}\n\n{:#?}\n\naddr={:#?}",
		stack_frame,
		fault,
		::x86_64::registers::control::Cr2::read()
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_invalid_tss(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: INVALID TSS --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_missing_segment(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: MISSING SEGMENT --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_stack_fault(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: STACK FAULT --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_gpf(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: GENERAL PROTECTION FAULT --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_chk_alignment(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: ALIGNMENT ERROR --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_chk_machine(stack_frame: InterruptStackFrame) -> ! {
	println!("\n\n-- ORO EXCEPTION: MACHINE ERROR --\n{:#?}", stack_frame);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_virtualization(stack_frame: InterruptStackFrame) {
	println!(
		"\n\n-- ORO EXCEPTION: VIRTUALIZATION ERROR --\n{:#?}",
		stack_frame
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_vmm(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: VMM COMM ERROR --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}

#[allow(clippy::missing_docs_in_private_items)]
extern "x86-interrupt" fn irq_security(stack_frame: InterruptStackFrame, error_code: u64) {
	println!(
		"\n\n-- ORO EXCEPTION: VMM SECURITY EVENT --\n{:#?}\n\nerror code = {}",
		stack_frame, error_code
	);
	crate::halt();
}
