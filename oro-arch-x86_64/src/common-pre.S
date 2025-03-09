//!; Common macro definitions used throughout the project.

///; Sets the stack pointer to the kernel's stack
///; base, depending on the addressing level.
///;
///; Clobbers RAX.
.macro SET_KERNEL_STACK_BASE
	mov rax, cr4
	and rax, 0b1000000000000
	test rax, rax
	jz 150f

	//; - CR4.LA57=1 (5-level paging)
	mov rsp, {KERNEL_STACK_BASE_L5}
	jmp 151f

150:
	//; - CR4.LA57=0 (4-level paging)
	mov rsp, {KERNEL_STACK_BASE_L4}

151:
.endmacro

///; Asserts that the stack is aligned to a given value.
///;
///; Does nothing under debug mode.
.macro CHECK_STACK_ALIGNMENT_DEBUG alignment, test_value
	//; Save RSP, as well as RDI so we don't clobber it.
	push rsp
	push rdi

	//; Test the alignment
	mov rdi, [rsp+8]
	and rdi, (\alignment - 1)
	cmp rdi, \test_value
	je 100f

	//; Not aligned; there's a bug!
	mov rsi, \alignment
	mov rcx, \test_value
	mov rdx, [rsp+8]
	call 101f
101:
	pop r8
	SET_KERNEL_STACK_BASE //; !! clobbers rax !!
	jmp _oro_isr_dbg_stack_unaligned //; (got=rdi, alignment=rsi, stack_ptr=rdx, expected_value=rcx, rip=r8)
	ud2

	//; We're aligned; restore RDI
100:
	pop rdi
	add rsp, 8 //; remove `rsp` from the stack
.endmacro

///; No-op check stack. Invoked in release mode.
.macro CHECK_STACK_ALIGNMENT_NOOP alignment, test_value
.endmacro
