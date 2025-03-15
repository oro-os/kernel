//!; Oro global ISR stubs.
//!;
//!; These functions are jumped to by the individual vector
//!; ISRs, where they will either jump to a core dump handler
//!; or forward the exception / interrupt to the kernel for
//!; userspace processing.
.pushsection .text.oro_isr64
.code64

///; Pushes the remaining fields of `StackFrame` onto the stack.
///;
///; Does not set `rdi` (e.g. for calling a Rust function with the
///; pointer to the `StackFrame`).
.macro PUSH_REGISTERS CHECK_STACK_ALIGNMENT, ADDITIONAL_REGISTER_PUSH
	//; We should have exactly 7 items on the stack.
	\CHECK_STACK_ALIGNMENT 4096, (4096 - (8 * 7))

	//; GP registers
	push rdi
	push rax
	push rbx
	push rcx
	push rdx
	push rsi
	push rbp
	push r8
	push r9
	push r10
	push r11
	push r12
	push r13
	push r14
	push r15

	//; fsbase MSR
	mov ecx, 0xC0000100
	rdmsr
	shl rdx, 32
	or rax, rdx
	push rax

	//; gsbase MSR
	mov ecx, 0xC0000101
	rdmsr
	shl rdx, 32
	or rax, rdx
	push rax

	//; At this point, we should be 64-byte aligned.
	//; If not, we jump to the debug handler for
	//; this particular case. Then push any additional
	//; registers to the stack space we just allocated.
	\CHECK_STACK_ALIGNMENT 64, 0
	sub rsp, 64 * 32
	\ADDITIONAL_REGISTER_PUSH
.endmacro

.macro DEFINE_HANDLERS CHECK_STACK_ALIGNMENT, exc_name, no_exc_name, ADDITIONAL_REGISTER_PUSH
	///; Common exception vector
	///;
	///; Checks if the incoming interrupt is coming from a kernel
	///; code segment (CS) and will jump to a core dump handler
	///; if so (as any exception whilst in the kernel code segment
	///; is a fatal panic scenario).
	///;
	///; The ISR stubs have already pushed a synthetic error code
	///; (if needed) and vector number to the stack.
	.global \exc_name
	\exc_name :
		PUSH_REGISTERS \CHECK_STACK_ALIGNMENT, \ADDITIONAL_REGISTER_PUSH

		//; Store the `StackFrame` into the first argument of the handler call.
		mov rdi, rsp

		//; Test the CS for the DPL
		mov rdx, [rsp + {CS_OFFSET}]

		//; We're done referencing the shadow stack, and it's necessary
		//; that in either case - core dump or forward - we need to set
		//; the kernel stack.
		SET_KERNEL_STACK_BASE //; !! clobbers rax !!

		//; Check for DPL=3 and skip over core dump if it's hit.
		and rdx, 3
		cmp rdx, 3
		je 4f

		//; - Ring 0 (we should panic)
		\CHECK_STACK_ALIGNMENT 16, 0
		jmp _oro_isr_rust_core_panic //; (stack_frame_ptr=rdi)
		ud2

		//; - Ring 3 (forward to kernel; no panic)
	4:
		\CHECK_STACK_ALIGNMENT 16, 0
		jmp _oro_isr_rust_handler //; (stack_frame_ptr=rdi)
		ud2

	///; Common non-exception vector.
	///;
	///; These are simply vectors that come from various
	///; events in the system and do not indicate an error.
	///; The CS's DPL indicator makes no difference.
	.global \no_exc_name
	\no_exc_name :
		PUSH_REGISTERS \CHECK_STACK_ALIGNMENT, \ADDITIONAL_REGISTER_PUSH

		//; Store the `StackFrame` into the first argument of the handler call.
		mov rdi, rsp

		//; If this is NOT the kernel, our stack is
		//; currently the thread's shadow stack; we
		//; need to set the stack to the kernel's
		//; stack base so it has some room to work.
		mov rax, [rsp + {CS_OFFSET}]
		and rax, 3
		cmp rax, 3
		jne 4f

		//; - Ring 3 (we should switch the stack)
		SET_KERNEL_STACK_BASE //; !! clobbers rax !!

		//; - Ring 0 (we're already on the kernel stack).
	4:
		\CHECK_STACK_ALIGNMENT 16, 0
		jmp _oro_isr_rust_handler //; (stack_frame_ptr=rdi)
		ud2
.endmacro

///; Pops the non-trampoline fields of `StackFrame` back into their registers.
.macro POP_REGISTERS CHECK_STACK_ALIGNMENT, ADDITIONAL_REGISTER_POP
	//; If vector registers are being preserved, restore them.
	\ADDITIONAL_REGISTER_POP
	add rsp, 64 * 32
	\CHECK_STACK_ALIGNMENT 64, 0

	//; gsbase MSR
	pop rdx
	mov eax, edx
	shr rdx, 32
	mov ecx, 0xC0000101
	wrmsr

	//; fsbase MSR
	pop rdx
	mov eax, edx
	shr rdx, 32
	mov ecx, 0xC0000100
	wrmsr

	//; GP registers
	pop r15
	pop r14
	pop r13
	pop r12
	pop r11
	pop r10
	pop r9
	pop r8
	pop rbp
	pop rsi
	pop rdx
	pop rcx
	pop rbx
	pop rax
	pop rdi
.endmacro

///; Defines the `iret` handler for user tasks.
.macro DEFINE_IRET CHECK_STACK_ALIGNMENT, iret_name ADDITIONAL_REGISTER_POP
	.global \iret_name
	\iret_name : //; fn(l4_page_phys = rdi, irq_frame_base = rsi)
		//; Switch to the task's page tables.
		mov cr3, rdi
		mov rsp, rsi

		//; Restore all registers.
		\CHECK_STACK_ALIGNMENT 16, 0
		POP_REGISTERS \CHECK_STACK_ALIGNMENT \ADDITIONAL_REGISTER_POP

		//; Pop off IV and Err
		add rsp, 16

		//; At this point, we should have exactly the following on the stack:
		//; ip, cs, flags, sp, ss
		\CHECK_STACK_ALIGNMENT 4096, (4096 - 40)

		//; "Return" from interrupt.
		iretq
		ud2
.endmacro

///; Main definition macro; invoked by the Rust side of things with the
///; given debug/release parameters.
.macro DEFINE_ALL_HANDLERS CHECK_STACK_ALIGNMENT
	DEFINE_HANDLERS \CHECK_STACK_ALIGNMENT, _oro_isr_common_exc, _oro_isr_common
	DEFINE_IRET \CHECK_STACK_ALIGNMENT, _oro_isr_iret
.endmacro

.popsection
