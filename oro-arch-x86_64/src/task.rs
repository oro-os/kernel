//! Task (context) switching routines.

use crate::mem::address_space::AddressSpaceLayout;
use core::arch::asm;
use oro_mem::mapper::{AddressSegment, AddressSpace};

/// Initializes a thread's IRQ stack, priming it for
/// its first context switch.
///
/// Returns the number of _bytes_ written.
pub fn initialize_user_irq_stack(page_slice: &mut [u64], entry_point: u64) -> u64 {
	// TODO(qix-): Not happy about doing this here. There should be a better way
	// TODO(qix-): to defend against fragmentation regarding this.
	let thread_stack_top = AddressSpaceLayout::module_thread_stack().range().1 & !0xFFF;

	let mut top = page_slice.len();
	let mut written = 0;
	let mut write_u64 = |val| {
		top -= 1;
		written += 8;
		page_slice[top] = val;
	};

	// Values for iretq
	write_u64((crate::gdt::USER_DS | 3).into()); // ds
	write_u64(thread_stack_top as u64); // rsp
	write_u64(crate::asm::rflags() | 0x200); // rflags
	write_u64((crate::gdt::USER_CS | 3).into()); // cs
	write_u64(entry_point); // rip

	// General purpose registers
	write_u64(0); // rax
	write_u64(0); // rbx
	write_u64(0); // rcx
	write_u64(0); // rdx
	write_u64(0); // rsi
	write_u64(0); // rdi
	write_u64(0); // rbp
	write_u64(0); // r8
	write_u64(0); // r9
	write_u64(0); // r10
	write_u64(0); // r11
	write_u64(0); // r12
	write_u64(0); // r13
	write_u64(0); // r14
	write_u64(0); // r15

	written
}

/// Switches from the kernel to a user task,
/// storing the kernel's state, restoring the
/// user task state, and resuming executing
/// via the `iretq` method.
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// - `rax` must be the physical address of the
///   user task's CR3.
/// - `rdx` must be the user task's IRQ stack pointer.
/// - `r9` must be a pointer to the core state `kernel_stack` field.
/// - `call` must be used to jump to this function.
///
/// All registers must be marked as clobbered.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// # Safety
/// This method is inherently unsafe.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
///
/// This function MUST NOT be jumped to; it must
/// always be called normally.
#[no_mangle]
#[naked]
pub unsafe extern "C" fn oro_x86_64_kernel_to_user() {
	// Push all general purpose registers
	// and then store the stack state.
	asm!(
		// "push rax", -- Not needed; `rax` is clobbered anyway.
		"push rbx",
		"push rcx",
		// "push rdx", -- Not needed; `rdx` is clobbered anyway.
		"push rsi",
		"push rdi",
		"push rbp",
		"push r8",
		// "push r9", -- Not needed; `r9` is clobbered anyway.
		"push r10",
		"push r11",
		"push r12",
		"push r13",
		"push r14",
		"push r15",
		"pushfq",
		"mov r11, rsp",
		"mov [r9], r11",
		"mov cr3, rax",
		"mov rsp, rdx",
		"mov ax, {}",
		"mov ds, ax",
		"mov es, ax",
		"mov fs, ax",
		"mov gs, ax",
		"pop r15",
		"pop r14",
		"pop r13",
		"pop r12",
		"pop r11",
		"pop r10",
		"pop r9",
		"pop r8",
		"pop rbp",
		"pop rdi",
		"pop rsi",
		"pop rdx",
		"pop rcx",
		"pop rbx",
		"pop rax",
		"sti",
		"iretq",
		const (crate::gdt::USER_DS | 3),
		options(noreturn)
	);
}

/// Switches from one user task to another,
/// WITHOUT storing any state, but restoring the
/// user task state, and resuming executing
/// via the `iretq` method.
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// - `rax` must be the physical address of the
///   user task's CR3.
/// - `rdx` must be the user task's IRQ stack pointer.
/// - `jmp` must be used to jump to this function.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// # Safety
/// This method is inherently unsafe.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
///
/// This function MUST NOT be called; it must
/// always be jumped to.
#[no_mangle]
#[naked]
pub unsafe extern "C" fn oro_x86_64_user_to_user() {
	// Push all general purpose registers
	// and then store the stack state.
	asm!(
		"mov cr3, rax",
		"mov rsp, rdx",
		"mov ax, {}",
		"mov ds, ax",
		"mov es, ax",
		"mov fs, ax",
		"mov gs, ax",
		"pop r15",
		"pop r14",
		"pop r13",
		"pop r12",
		"pop r11",
		"pop r10",
		"pop r9",
		"pop r8",
		"pop rbp",
		"pop rdi",
		"pop rsi",
		"pop rdx",
		"pop rcx",
		"pop rbx",
		"pop rax",
		"sti",
		"iretq",
		const (crate::gdt::USER_DS | 3),
		options(noreturn)
	);
}

/// Stores the user task's state in order to process an interrupt.
///
/// **This does not restore the kernel's core thread; it ONLY
/// stores the user task's state so that general purpose registers
/// are not clobbered.**
///
/// That function MUST NOT return (at least, not back to the ISR
/// stub).
///
/// The function MUST store `rcx` as the thread's new IRQ
/// stack pointer, at the very start of the function, before
/// any other register clobbering may occur.
///
/// To be used **solely** from ISR stubs. This macro will disable
/// interrupts for you.
///
/// The function must be provided as an identifier.
#[macro_export]
macro_rules! isr_store_user_task_and_jmp {
	($jmp_to:ident) => {
		asm!(
			"cli",
			"push rax",
			"push rbx",
			"push rcx",
			"push rdx",
			"push rsi",
			"push rdi",
			"push rbp",
			"push r8",
			"push r9",
			"push r10",
			"push r11",
			"push r12",
			"push r13",
			"push r14",
			"push r15",
			"mov rcx, rsp",
			concat!("jmp ", stringify!($jmp_to)),
			"ud2",
			options(noreturn)
		);
	};
}
