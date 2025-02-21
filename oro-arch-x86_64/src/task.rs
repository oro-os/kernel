//! Task (context) switching routines.

use core::arch::naked_asm;

/// Initializes a thread's IRQ stack, priming it for
/// its first context switch.
///
/// Returns the number of _bytes_ written.
pub fn initialize_user_irq_stack(page_slice: &mut [u64], entry_point: u64, stack_ptr: u64) -> u64 {
	let mut top = page_slice.len();
	let mut written = 0;
	let mut write_u64 = |val| {
		top -= 1;
		written += 8;
		page_slice[top] = val;
	};

	// Values for iretq
	write_u64((crate::gdt::USER_DS | 3).into()); // ds
	write_u64(stack_ptr); // rsp
	// TODO(qix-): Generate a well-specified RFLAGS value after an RFLAGS register type is created.
	write_u64(crate::asm::rflags() | 0x200); // rflags (including IF)
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
	write_u64(0); // rflags

	written
}

/// Switches from the kernel to a user task,
/// storing the kernel's state, restoring the
/// user task state, and resuming executing
/// via the `iretq` method.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// - `rax` must be the physical address of the
///   user task's CR3. All core-local mappings
///   must be transferred to the mapper prior to this
///   function being called.
/// - `rdx` must be the user task's IRQ stack pointer.
/// - `r9` must be a pointer to the core state `kernel_irq_stack` field.
/// - `r10` must be a pointer to the core state `kernel_stack` field.
/// - `call` must be used to jump to this function.
///
/// All registers must be marked as clobbered.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_kernel_to_user() {
	// Push all general purpose registers
	// and then store the stack state.
	naked_asm!(
		"mov [r10], rsp",
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
		"pushfq",
		"mov r11, rsp",
		"mov [r9], r11",
		"mov cr3, rax",
		"mov rsp, rdx",
		"mov ax, {}",
		"mov ds, ax",
		"mov es, ax",
		"popfq",
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
		"iretq",
		const (crate::gdt::USER_DS | 3),
	);
}

/// Switches from the kernel to a user task,
/// responding to a syscall.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// **This is an incredibly sensitive security boundary.** Calls to this
/// function should be done with extreme caution and care.
///
/// Caller must ensure that the task is ready to be executed again. This means
/// the memory map, core-local mappings, etc. are all restored to the task that
/// originally made the syscall.
///
/// This function **must only be called** from
/// a task that was previously switched *away* from
/// under system call circumstances.
///
/// - `r8` must be the `cr3` value of the user task.
/// - `r10` must be the user task's IRQ stack pointer.
/// - `rdi` must be a pointer to the core state `kernel_irq_stack` field.
/// - `rsi` must be a pointer to the core state `kernel_stack` field.
/// - `rax` must contain the system call error code.
/// - `r9` must contain the system call return value.
/// - `call` must be used to jump to this function.
///
/// All registers must be marked as clobbered.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_kernel_to_user_sysret() {
	// TODO(qix-): There is almost definitely some missing functionality here, namely
	// TODO(qix-): around the resume flag (RF) and the trap flag (TF) in the RFLAGS register.
	naked_asm!(
		"mov [rsi], rsp",
		"push r8",
		"push r9",
		"push r10",
		"push r11",
		"push rdi",
		"push rax",
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
		"pushfq",
		"mov r11, rsp",
		"mov [rdi], r11",
		"mov cr3, r8",
		"mov rsp, r10",
		"pop rcx", // RIP
		// Force the return address to a canonical address
		"and rcx, ORO_SYSCALL_CANONICAL_ADDRESS_MASK",
		"pop r11", // RFLAGS
		"pop rbp",
		"pop rbx",
		"pop r12",
		"pop r13",
		"pop r14",
		"pop r15",
		"pop rsp",
		// Zero clobbered registers.
		//
		// SAFETY(qix-): Vector registers are clobbered AND considered insecurely transferred.
		// SAFETY(qix-): It is specified that the kernel DOES NOT zero vector registers.
		"xor rdx, rdx",
		"xor r8, r8",
		"xor r10, r10",
		"xor rdi, rdi",
		"xor rsi, rsi",
		// Return to userspace.
		"sysretq",
	);
}

/// Switches from a user task to another user task,
/// WITHOUT storing any state, and returning to the
/// given task via a system call return.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// jumped to from an `asm!()` block.**
///
/// **This is an incredibly sensitive security boundary.** Calls to this
/// function should be done with extreme caution and care.
///
/// Caller must ensure that the task is ready to be executed again. This means
/// the memory map, core-local mappings, etc. are all restored to the task that
/// originally made the syscall.
///
/// This function **must only be jumped to** from
/// a task that was previously switched *away* from
/// under system call circumstances.
///
/// - `r8` must be the `cr3` value of the user task.
/// - `rax` must contain the system call error code.
/// - `r9` must contain the system call return value.
/// - `r10` must be the user task's IRQ stack pointer.
///
/// All registers must be marked as clobbered.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_user_to_user_sysret() {
	// TODO(qix-): There is almost definitely some missing functionality here, namely
	// TODO(qix-): around the resume flag (RF) and the trap flag (TF) in the RFLAGS register.
	naked_asm! {
		"mov cr3, r8",
		"mov rsp, r10",
		"pop rcx", // RIP
		// Force the return address to a canonical address
		"and rcx, ORO_SYSCALL_CANONICAL_ADDRESS_MASK",
		"pop r11", // RFLAGS
		"pop rbp",
		"pop rbx",
		"pop r12",
		"pop r13",
		"pop r14",
		"pop r15",
		"pop rsp",
		// Zero clobbered registers.
		//
		// SAFETY(qix-): Vector registers are clobbered AND considered insecurely transferred.
		// SAFETY(qix-): It is specified that the kernel DOES NOT zero vector registers.
		"xor rdx, rdx",
		"xor r8, r8",
		"xor r10, r10",
		"xor rdi, rdi",
		"xor rsi, rsi",
		// Return to userspace.
		"sysretq",
	}
}

/// Stores the task's state in order to process a system call.
///
/// **This does not restore the kernel's core thread; it ONLY
/// stores the calling task's state so that general purpose registers
/// are not clobbered.**
///
/// **This must not be called from kernel contexts. The Oro kernel
/// does not support `syscall` instructions from kernel contexts!**
///
/// The function is called with the task's IRQ stack loaded into `rsp`.
/// The stack can be used briefly to switch to another task, but
/// anything after `r8` should be considered trash after the task
/// is switched away from.
///
/// That function MUST NOT return (at least, not back to the ISR
/// stub).
///
/// The function MUST store the following at the very start of the
/// function, before any other register clobbering may occur:
///
/// - `r8` as the thread's new IRQ stack pointer.
/// - 'rax' as the system call opcode
/// - 'rsi' as the system call table ID
/// - 'rdi' as the system call key
/// - 'rdx' as the system call value
/// - 'r9' as the system call entity ID
///
/// To be used **solely** from syscall entry stubs. Interrupts
/// are disabled already by the `syscall` instruction.
///
/// The function must be provided as an identifier.
#[macro_export]
macro_rules! syscall_store_task_and_jmp {
	($jmp_to:ident) => {
		naked_asm! {
			// Store the user stack for a moment.
			"mov r8, rsp",
			// Set the stack base pointer to the IRQ stack base address.
			"mov rsp, ORO_SYSCALL_IRQ_STACK_BASE",
			// Push all values needed by the syscall return stub
			// to the stack.
			"push r8", // user task stack pointer
			"push r15",
			"push r14",
			"push r13",
			"push r12",
			"push rbx",
			"push rbp",
			"push r11", // RFLAGS
			"push rcx", // RIP
			// Now store the stack pointer.
			"mov r8, rsp",
			// Jump to the handler
			concat!("jmp ", stringify!($jmp_to)),
			"ud2",
		}
	};
}

/// Switches from one user task to another,
/// WITHOUT storing any state, but restoring the
/// user task state, and resuming executing
/// via the `iretq` method.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// jumped to from an `asm!()` block.**
///
/// - `rax` must be the physical address of the
///   user task's CR3. All core-local mappings
///   must be transferred to the mapper prior to this
///   function being called.
/// - `rdx` must be the user task's IRQ stack pointer.
/// - `jmp` must be used to jump to this function.
///
/// The HEAD of the task's IRQ stack must be stored
/// in `Tss::rsp0` before calling this function.
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
///
/// This function MUST NOT be called; it must
/// always be jumped to.
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_user_to_user() {
	// Push all general purpose registers
	// and then store the stack state.
	naked_asm!(
		"mov cr3, rax",
		"mov rsp, rdx",
		"mov ax, {}",
		"mov ds, ax",
		"mov es, ax",
		"popfq",
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
		"iretq",
		const (crate::gdt::USER_DS | 3),
	);
}

/// Stores the task's state in order to process an interrupt.
///
/// **This does not restore the kernel's core thread; it ONLY
/// stores the calling task's state so that general purpose registers
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
macro_rules! isr_store_task_and_jmp {
	($jmp_to:ident) => {
		::core::arch::naked_asm!(
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
			"pushfq",
			"mov rcx, rsp",
			concat!("jmp ", stringify!($jmp_to)),
			"ud2",
		);
	};
}

/// [`isr_store_task_and_jmp`] but with an error code stored to `rdx`.
///
/// All the same rules apply.
#[macro_export]
macro_rules! isr_store_task_and_jmp_err {
	($jmp_to:ident) => {
		::core::arch::naked_asm!(
			"cli",
			// RDX is what we're using for the error code.
			// However, it currently holds an application's value.
			// We need to save it before we clobber it.
			"push rdx",
			// Now we can load the error code into RDX.
			"mov rdx, [rsp + 8]",
			// We can't pop twice, and a sub is needless here,
			// so for the two first "pushes", we store directly
			// to the stack.
			"mov [rsp + 8], rax",
			"mov [rsp], rbx",
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
			"pushfq",
			"mov rcx, rsp",
			concat!("jmp ", stringify!($jmp_to)),
			"ud2",
		);
	};
}

/// Pops the kernel state from the stack and returns to the kernel.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// - `r9` must be the kernel's stack prior to the context switch.
/// - `jmp` must be used to jump to this function.
///
/// **`r9` must be marked as clobbered at the callsite from which
/// the kernel context was switched!**
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
///
/// This function MUST NOT be called; it must
/// always be jumped to.
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_return_to_kernel() {
	naked_asm!(
		"popfq",
		"pop r15",
		"pop r14",
		"pop r13",
		"pop r12",
		"pop r11",
		"pop r10",
		"add rsp, 8", // skip r9; it's common-clobbered and an input to this function.
		"pop r8",
		"pop rbp",
		"pop rdi",
		"pop rsi",
		"pop rdx",
		"pop rcx",
		"pop rbx",
		"pop rax",
		"mov rsp, r9",
		"ret",
	);
}

/// Pushes the kernel state and halts the core, waiting
/// for an interrupt.
///
/// # Safety
///
/// **This is not a normal function. It must be
/// called from an `asm!()` block.**
///
/// - `r9` must be the kernel's stack pointer.
/// - `call` must be used to jump to this function.
///
/// **The `asm!()` call MUST declare `r9` as clobbered!**
///
/// Caller MUST NOT have any critical sections
/// enabled, or any locks held.
///
/// **Interrupts must be disabled before calling
/// this function.**
///
/// This function MUST NOT be jumped to; it must
/// always be called normally.
#[unsafe(no_mangle)]
#[naked]
pub unsafe extern "C" fn oro_x86_64_kernel_to_idle() {
	naked_asm!("mov [r9], rsp", "sti", "4: hlt", "jmp 4b", "ud2",);
}
