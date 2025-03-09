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

///; Pushes the AVX-512 vector registers to the stack.
.macro PUSH_ZMM_REGISTERS
	//; Each of the ZMMn registers.
	vmovdqa64 [rsp + (31 * 64)], zmm31
	vmovdqa64 [rsp + (30 * 64)], zmm30
	vmovdqa64 [rsp + (29 * 64)], zmm29
	vmovdqa64 [rsp + (28 * 64)], zmm28
	vmovdqa64 [rsp + (27 * 64)], zmm27
	vmovdqa64 [rsp + (26 * 64)], zmm26
	vmovdqa64 [rsp + (25 * 64)], zmm25
	vmovdqa64 [rsp + (24 * 64)], zmm24
	vmovdqa64 [rsp + (23 * 64)], zmm23
	vmovdqa64 [rsp + (22 * 64)], zmm22
	vmovdqa64 [rsp + (21 * 64)], zmm21
	vmovdqa64 [rsp + (20 * 64)], zmm20
	vmovdqa64 [rsp + (19 * 64)], zmm19
	vmovdqa64 [rsp + (18 * 64)], zmm18
	vmovdqa64 [rsp + (17 * 64)], zmm17
	vmovdqa64 [rsp + (16 * 64)], zmm16
	vmovdqa64 [rsp + (15 * 64)], zmm15
	vmovdqa64 [rsp + (14 * 64)], zmm14
	vmovdqa64 [rsp + (13 * 64)], zmm13
	vmovdqa64 [rsp + (12 * 64)], zmm12
	vmovdqa64 [rsp + (11 * 64)], zmm11
	vmovdqa64 [rsp + (10 * 64)], zmm10
	vmovdqa64 [rsp + (9 * 64)], zmm9
	vmovdqa64 [rsp + (8 * 64)], zmm8
	vmovdqa64 [rsp + (7 * 64)], zmm7
	vmovdqa64 [rsp + (6 * 64)], zmm6
	vmovdqa64 [rsp + (5 * 64)], zmm5
	vmovdqa64 [rsp + (4 * 64)], zmm4
	vmovdqa64 [rsp + (3 * 64)], zmm3
	vmovdqa64 [rsp + (2 * 64)], zmm2
	vmovdqa64 [rsp + (1 * 64)], zmm1
	vmovdqa64 [rsp], zmm0
.endmacro

///; Pushes the AVX vector registers to the stack.
.macro PUSH_YMM_REGISTERS
	//; Each of the YMMn registers.
	vmovdqa [rsp + (15 * 32)], ymm15
	vmovdqa [rsp + (14 * 32)], ymm14
	vmovdqa [rsp + (13 * 32)], ymm13
	vmovdqa [rsp + (12 * 32)], ymm12
	vmovdqa [rsp + (11 * 32)], ymm11
	vmovdqa [rsp + (10 * 32)], ymm10
	vmovdqa [rsp + (9 * 32)], ymm9
	vmovdqa [rsp + (8 * 32)], ymm8
	vmovdqa [rsp + (7 * 32)], ymm7
	vmovdqa [rsp + (6 * 32)], ymm6
	vmovdqa [rsp + (5 * 32)], ymm5
	vmovdqa [rsp + (4 * 32)], ymm4
	vmovdqa [rsp + (3 * 32)], ymm3
	vmovdqa [rsp + (2 * 32)], ymm2
	vmovdqa [rsp + (1 * 32)], ymm1
	vmovdqa [rsp], ymm0
.endmacro

///; Pushes the SSE vector registers to the stack.
.macro PUSH_XMM_REGISTERS
	movdqa [rsp + (15 * 16)], xmm15
	movdqa [rsp + (14 * 16)], xmm14
	movdqa [rsp + (13 * 16)], xmm13
	movdqa [rsp + (12 * 16)], xmm12
	movdqa [rsp + (11 * 16)], xmm11
	movdqa [rsp + (10 * 16)], xmm10
	movdqa [rsp + (9 * 16)], xmm9
	movdqa [rsp + (8 * 16)], xmm8
	movdqa [rsp + (7 * 16)], xmm7
	movdqa [rsp + (6 * 16)], xmm6
	movdqa [rsp + (5 * 16)], xmm5
	movdqa [rsp + (4 * 16)], xmm4
	movdqa [rsp + (3 * 16)], xmm3
	movdqa [rsp + (2 * 16)], xmm2
	movdqa [rsp + (1 * 16)], xmm1
	movdqa [rsp], xmm0
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

///; Pops the AVX-512 vector registers from the stack.
.macro POP_ZMM_REGISTERS
	//; Each of the ZMMn registers.
	vmovdqa64 zmm31, [rsp + (31 * 64)]
	vmovdqa64 zmm30, [rsp + (30 * 64)]
	vmovdqa64 zmm29, [rsp + (29 * 64)]
	vmovdqa64 zmm28, [rsp + (28 * 64)]
	vmovdqa64 zmm27, [rsp + (27 * 64)]
	vmovdqa64 zmm26, [rsp + (26 * 64)]
	vmovdqa64 zmm25, [rsp + (25 * 64)]
	vmovdqa64 zmm24, [rsp + (24 * 64)]
	vmovdqa64 zmm23, [rsp + (23 * 64)]
	vmovdqa64 zmm22, [rsp + (22 * 64)]
	vmovdqa64 zmm21, [rsp + (21 * 64)]
	vmovdqa64 zmm20, [rsp + (20 * 64)]
	vmovdqa64 zmm19, [rsp + (19 * 64)]
	vmovdqa64 zmm18, [rsp + (18 * 64)]
	vmovdqa64 zmm17, [rsp + (17 * 64)]
	vmovdqa64 zmm16, [rsp + (16 * 64)]
	vmovdqa64 zmm15, [rsp + (15 * 64)]
	vmovdqa64 zmm14, [rsp + (14 * 64)]
	vmovdqa64 zmm13, [rsp + (13 * 64)]
	vmovdqa64 zmm12, [rsp + (12 * 64)]
	vmovdqa64 zmm11, [rsp + (11 * 64)]
	vmovdqa64 zmm10, [rsp + (10 * 64)]
	vmovdqa64 zmm9, [rsp + (9 * 64)]
	vmovdqa64 zmm8, [rsp + (8 * 64)]
	vmovdqa64 zmm7, [rsp + (7 * 64)]
	vmovdqa64 zmm6, [rsp + (6 * 64)]
	vmovdqa64 zmm5, [rsp + (5 * 64)]
	vmovdqa64 zmm4, [rsp + (4 * 64)]
	vmovdqa64 zmm3, [rsp + (3 * 64)]
	vmovdqa64 zmm2, [rsp + (2 * 64)]
	vmovdqa64 zmm1, [rsp + (1 * 64)]
	vmovdqa64 zmm0, [rsp]
.endmacro

///; Pops the AVX vector registers from the stack.
.macro POP_YMM_REGISTERS
	//; Each of the YMMn registers.
	vmovdqa ymm15, [rsp + (15 * 32)]
	vmovdqa ymm14, [rsp + (14 * 32)]
	vmovdqa ymm13, [rsp + (13 * 32)]
	vmovdqa ymm12, [rsp + (12 * 32)]
	vmovdqa ymm11, [rsp + (11 * 32)]
	vmovdqa ymm10, [rsp + (10 * 32)]
	vmovdqa ymm9, [rsp + (9 * 32)]
	vmovdqa ymm8, [rsp + (8 * 32)]
	vmovdqa ymm7, [rsp + (7 * 32)]
	vmovdqa ymm6, [rsp + (6 * 32)]
	vmovdqa ymm5, [rsp + (5 * 32)]
	vmovdqa ymm4, [rsp + (4 * 32)]
	vmovdqa ymm3, [rsp + (3 * 32)]
	vmovdqa ymm2, [rsp + (2 * 32)]
	vmovdqa ymm1, [rsp + (1 * 32)]
	vmovdqa ymm0, [rsp]
.endmacro

///; Pops the SSE vector registers to the stack.
.macro POP_XMM_REGISTERS
	movdqa xmm15, [rsp + (15 * 16)]
	movdqa xmm14, [rsp + (14 * 16)]
	movdqa xmm13, [rsp + (13 * 16)]
	movdqa xmm12, [rsp + (12 * 16)]
	movdqa xmm11, [rsp + (11 * 16)]
	movdqa xmm10, [rsp + (10 * 16)]
	movdqa xmm9, [rsp + (9 * 16)]
	movdqa xmm8, [rsp + (8 * 16)]
	movdqa xmm7, [rsp + (7 * 16)]
	movdqa xmm6, [rsp + (6 * 16)]
	movdqa xmm5, [rsp + (5 * 16)]
	movdqa xmm4, [rsp + (4 * 16)]
	movdqa xmm3, [rsp + (3 * 16)]
	movdqa xmm2, [rsp + (2 * 16)]
	movdqa xmm1, [rsp + (1 * 16)]
	movdqa xmm0, [rsp]
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
	DEFINE_HANDLERS \CHECK_STACK_ALIGNMENT, _oro_isr_common_exc_zmm, _oro_isr_common_zmm, PUSH_ZMM_REGISTERS
	DEFINE_HANDLERS \CHECK_STACK_ALIGNMENT, _oro_isr_common_exc_ymm, _oro_isr_common_ymm, PUSH_YMM_REGISTERS
	DEFINE_HANDLERS \CHECK_STACK_ALIGNMENT, _oro_isr_common_exc_xmm, _oro_isr_common_xmm, PUSH_XMM_REGISTERS
	DEFINE_HANDLERS \CHECK_STACK_ALIGNMENT, _oro_isr_common_exc_novec, _oro_isr_common_novec
	DEFINE_IRET \CHECK_STACK_ALIGNMENT, _oro_isr_iret_zmm, POP_ZMM_REGISTERS
	DEFINE_IRET \CHECK_STACK_ALIGNMENT, _oro_isr_iret_ymm, POP_YMM_REGISTERS
	DEFINE_IRET \CHECK_STACK_ALIGNMENT, _oro_isr_iret_xmm, POP_XMM_REGISTERS
	DEFINE_IRET \CHECK_STACK_ALIGNMENT, _oro_isr_iret_novec
.endmacro

.popsection
