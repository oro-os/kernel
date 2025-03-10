//!; Syscall handlers and sysret implementation for x86_64.
.pushsection .text.oro_syscall
.code64

.macro DEFINE_SYSCALL_HANDLERS CHECK_STACK_ALIGNMENT
	///; Main handler for syscalls.
	///;
	///; All invocations of this subroutine are from DPL=3;
	///; the kernel never uses `syscall`.
	.global _oro_syscall_enter
	_oro_syscall_enter:
		//; NOTE(qix-): We don't need to `cli`/`cld`; CPU already
		//; does that for us (we specify it in the flags mask
		//; during `syscall` installation; see `syscall/mod.rs`.

		//; First, we need to set the IRQ stack base.
		//; We also store the user stack in r8 (documented
		//; as clobbered).
		mov r8, rsp
		mov rsp, ORO_SYSCALL_IRQ_STACK_BASE
		\CHECK_STACK_ALIGNMENT 4096, 0

		//; We need to construct a `StackFrame` but only populate
		//; the registers that actually matter for the syscall.
		//; The Oro ABI dictates that many (most) registers are
		//; NOT preserved over the course of the syscall, including
		//; vector registers, etc.
		//;
		//; In accordance with the IRQ system, the stack should
		//; be aligned to a 64 byte boundary afterward.
		sub rsp, {STACK_FRAME_SIZE}
		\CHECK_STACK_ALIGNMENT 64, 0

		//; Now populate the registers needed by the system call.
		mov QWORD PTR [rsp + {CS_OFFSET}], {USER_CS}
		mov QWORD PTR [rsp + {SS_OFFSET}], {USER_SS}
		mov [rsp + {SP_OFFSET}], r8
		mov [rsp + {R15_OFFSET}], r15
		mov [rsp + {R14_OFFSET}], r14
		mov [rsp + {R13_OFFSET}], r13
		mov [rsp + {R12_OFFSET}], r12
		mov [rsp + {R9_OFFSET}], r9
		mov [rsp + {RAX_OFFSET}], rax
		mov [rsp + {RSI_OFFSET}], rsi
		mov [rsp + {RDI_OFFSET}], rdi
		mov [rsp + {RDX_OFFSET}], rdx
		mov [rsp + {RBP_OFFSET}], rbp
		mov [rsp + {RBX_OFFSET}], rbx
		mov [rsp + {FLAGS_OFFSET}], r11 //; x86_64 puts RFLAGS into R11 before jumping here
		mov [rsp + {IP_OFFSET}], rcx //; similarly, the original RIP goes into RCX

		//; Further, the IV is set to `!0` to indicate a system call.
		mov QWORD PTR [rsp + {IV_OFFSET}], 0xFFFFFFFFFFFFFFFF

		//; We have now fully initialized the stack frame; it may hold "garbage"
		//; data but only ever from the currently running task's previous time slices.
		//;
		//; We can now move the stack pointer into `rdi` for the first argument
		//; of the syscall handler.
		mov rdi, rsp

		//; Install the kernel's stack again at its base.
		SET_KERNEL_STACK_BASE //; !! clobbers rax !!!

		//; Now we're ready to jump.
		jmp _oro_syscall_handler
		ud2

	///; Handles returning to userspace via a `sysret` instruction.
	.global _oro_syscall_return
	_oro_syscall_return: //; fn(cr3=rdi, irq_frame_base=rsi, *const response: rdx)
		//; Set the page tables for the thread.
		mov cr3, rdi

		//; Set the stack to the base of the IRQ frame.
		mov rsp, rsi
		\CHECK_STACK_ALIGNMENT 4096, (4096 - {STACK_FRAME_SIZE})

		//; First, set the base pointer, We do this here because
		//; we're about to clobber RDX with our wrmsr instructions.
		mov rbp, rdx

		//; gsbase MSR (we do it here since it clobbers EAX)
		mov rdx, [rsp + {GSBASE_OFFSET}]
		mov eax, edx
		shr rdx, 32
		mov ecx, 0xC0000101
		wrmsr

		//; fsbase MSR
		mov rdx, [rsp + {FSBASE_OFFSET}]
		mov eax, edx
		shr rdx, 32
		mov ecx, 0xC0000100
		wrmsr

		//; Then, extract the bits from our response into the
		//; appropriate Oro ABI response registers. We do this
		//; *after* the wrmsr because they clobber rax.
		mov rax, [rbp + {SYS_RES_ERROR_OFFSET}]
		mov r9, [rbp + {SYS_RES_RET_OFFSET}]

		//; Now restore the registers
		mov r15, [rsp + {R15_OFFSET}]
		mov r14, [rsp + {R14_OFFSET}]
		mov r13, [rsp + {R13_OFFSET}]
		mov r12, [rsp + {R12_OFFSET}]
		mov rbp, [rsp + {RBP_OFFSET}]
		mov rbx, [rsp + {RBX_OFFSET}]
		mov r11, [rsp + {FLAGS_OFFSET}] //; x86_64 puts RFLAGS into R11 before jumping here
		mov rcx, [rsp + {IP_OFFSET}] //; similarly, the original RIP goes into RCX

		//; We're done with the stack now; restore it.
		mov r8, [rsp + {SP_OFFSET}]
		mov rsp, r8

		//; Ensure a canonical return address.
		and rcx, ORO_SYSCALL_CANONICAL_ADDRESS_MASK

		//; Zero clobbered registers.
		xor rdx, rdx
		xor r8, r8
		xor r10, r10
		xor rdi, rdi
		xor rsi, rsi

		//; Return
		sysretq
		ud2
.endmacro

.popsection
