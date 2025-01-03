//! Syscall handler implementation for x86_64.

use core::arch::{asm, naked_asm};

use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, UnmapError},
	pfa::Alloc,
};

use crate::{
	asm::{rdmsr, wrmsr},
	mem::paging_level::PagingLevel,
};

/// Core-local syscall stack base pointer.
///
/// Holds the base address of the core-local syscall stack, stored as `rsp` when
/// a system call is made from the userland as a temporary, tiny stack space.
///
/// This is a mutable static and not a constant since its value relies on the
/// paging mode, which can only be determined at runtime.
#[no_mangle]
static mut ORO_SYSCALL_STACK_BASE: u64 = 0;

/// Caches the system's paging level.
#[no_mangle]
static mut CANONICAL_ADDRESS_MASK: u64 = !0;

/// The number of pages in the core-local syscall stack to allocate.
/// Should be small - 1 or 2 pages should be sufficient.
const SYSCALL_STACK_PAGES: usize = 1;

/// Installs the syscall handler.
///
/// # Safety
/// Must be called _exactly once_ per core, at the beginning of each
/// core's lifetime.
///
/// # Panics
/// Panics if the core-local syscall stack guard pages are already mapped,
/// or if allocating / mapping the syscall stack pages fails.
pub unsafe fn install_syscall_handler() {
	// Enable the IA32_EFER.SCE bit, which enables syscall/sysret.
	// Otherwise, #UD will be raised when executing SYSCALL on
	// intel processors.
	let mut ia32_efer = rdmsr(0xC000_0080);
	ia32_efer |= 1; // Set the SCE bit
	wrmsr(0xC000_0080, ia32_efer);

	// Tell the CPU to clear the interrupt enable flag (IF) and trap flag (TF) when
	// executing a syscall via the SFMASK MSR.
	//
	// TODO(qix-): Make an RFLAGS register in crate::reg and use that instead
	// TODO(qix-): of hardcoding the value.
	wrmsr(0xC000_0084, 0x0200 | 0x0100);

	// Tell the CPU which CS and SS selectors to use when executing a syscall.
	// See the `STAR` constant in the `gid` module for more information.
	#[doc(hidden)]
	const STAR: u64 = ((must_be_u16(crate::gdt::STAR_KERNEL) as u64) << 32)
		| ((must_be_u16(crate::gdt::STAR_USER) as u64) << 48);

	wrmsr(0xC000_0081, STAR);

	// Tell the CPU where to jump to when a syscall is executed (LSTAR MSR).
	wrmsr(0xC000_0082, syscall_enter_non_compat as *const () as u64);

	// Finally, set up the syscall stack and store the base pointer.
	let stack_segment = crate::mem::address_space::AddressSpaceLayout::kernel_syscall_stack();
	unsafe {
		ORO_SYSCALL_STACK_BASE = u64::try_from(stack_segment.range().1 & !0xFFF).unwrap();
	}

	let kernel_mapper = crate::Kernel::get().mapper();

	// Ensure that the upper page is not mapped (guard page).
	let mut current_page = stack_segment.range().1 & !0xFFF;

	match stack_segment
		.unmap(kernel_mapper, current_page)
		.expect_err("core-local syscall stack upper guard page was already mapped")
	{
		UnmapError::NotMapped => {}
		err => {
			panic!(
				"core-local syscall stack upper guard page encountered error when unmapping: \
				 {err:?}"
			)
		}
	}

	// Allocate stack pages.
	for _ in 0..SYSCALL_STACK_PAGES {
		current_page -= 0x1000;
		stack_segment
			.map(
				kernel_mapper,
				current_page,
				GlobalPfa
					.allocate()
					.expect("failed to allocate core-local syscall stack page"),
			)
			.expect("failed to map core-local syscall stack page");
	}

	// Ensure the lower guard page is not mapped.
	current_page -= 0x1000;
	match stack_segment
		.unmap(kernel_mapper, current_page)
		.expect_err("core-local syscall stack lower guard page was already mapped")
	{
		UnmapError::NotMapped => {}
		err => {
			panic!(
				"core-local syscall stack lower guard page encountered error when unmapping: \
				 {err:?}"
			)
		}
	}

	// Get the paging level and construct a canonical address mask.
	CANONICAL_ADDRESS_MASK = match PagingLevel::current_from_cpu() {
		// 47-bit canonical addresses (lower-half 48 bit address)
		PagingLevel::Level4 => 0x0000_7FFF_FFFF_FFFF,
		// 56-bit canonical addresses (lower-half 57 bit address)
		PagingLevel::Level5 => 0x00FF_FFFF_FFFF_FFFF,
	};
}

/// A frame for an ABI call, for the x86_64 architecture.
#[derive(Clone, Copy)]
#[expect(clippy::missing_docs_in_private_items)]
pub struct AbiCallFrame {
	rax:    u64,
	rsi:    u64,
	rdi:    u64,
	rdx:    u64,
	r9:     u64,
	rbp:    u64,
	rsp:    u64,
	rbx:    u64,
	r12:    u64,
	r13:    u64,
	r14:    u64,
	r15:    u64,
	rcx:    u64,
	rflags: u64,
}

/// Entry point for 64-bit, non-compatibility-mode syscalls.
///
/// # Safety
/// **This is an incredibly sensitive security boundary.**
/// Maintenance on this function should be done with extreme caution and care.
///
/// Not to be called directly by kernel code.
///
/// It's also assumed that core-local top level indices are copied
/// into the thread's mapper before `cr3` is switched during a
/// context switch. This is, perhaps, the most dangerous and important
/// part of the entire kernel.
#[no_mangle]
#[naked]
unsafe extern "C" fn syscall_enter_non_compat() -> ! {
	naked_asm! {
		// First, store the userspace stack to r8 so that we can
		// store it in the AbiCallFrame later.
		"mov r8, rsp",
		// Then we substitute in a temporary, highly volatile,
		// very tiny core-local stack. We do this since we can't
		// trust the userspace stack to be in a good state or even
		// point to valid memory.
		//
		// In other kernels this is typically done with a `swapgs`,
		// but we don't have nor need a GS segment in Oro due to
		// limitations in the TLS codegen functionality in Rust/LLVM.
		// Thus, we've eschewed the use of GS/FS entirely and simply
		// allow userspace to use them as they see fit.
		"mov rsp, ORO_SYSCALL_STACK_BASE",
		"jmp syscall_enter_non_compat_stage2",
	}
}

/// The second stage of the 64-bit, non-compatibility-mode syscall entry.
#[no_mangle]
unsafe extern "C" fn syscall_enter_non_compat_stage2() -> ! {
	// We then construct our frame. For now, it'll have to be copied
	// to the new stack space, but we can optimize this later.
	//
	// NOTE(qix-): It is unendingly important that no registers are clobbered
	// NOTE(qix-): here. Thus, function calls are not allowed.
	let mut frame: AbiCallFrame = AbiCallFrame {
		rax:    0,
		rsi:    0,
		rdi:    0,
		rdx:    0,
		r9:     0,
		rbp:    0,
		rsp:    0,
		rbx:    0,
		r12:    0,
		r13:    0,
		r14:    0,
		r15:    0,
		rcx:    0,
		rflags: 0,
	};

	// Store all of the registers in the frame.
	asm! {
		"",
		out("rax") frame.rax,
		out("rsi") frame.rsi,
		out("rdi") frame.rdi,
		out("rdx") frame.rdx,
		out("r9") frame.r9,
		// R8 is the userspace stack pointer, which we've already stored.
		// RSP is instead the core-local syscall stack pointer, which we
		// don't need to save (and will be a nonsense value by this point,
		// anyway).
		out("r8") frame.rsp,
		out("r12") frame.r12,
		out("r13") frame.r13,
		out("r14") frame.r14,
		out("r15") frame.r15,
		// Holds the return address used by the eventual `sysret` or `iret`.
		out("rcx") frame.rcx,
		// The `syscall` instruction automatically puts `rflags` into `r11`.
		out("r11") frame.rflags,
		options(nostack),
	};

	// We cannot store `rbp` and `rbx` directly since LLVM treats them
	// specially when using inline assembly directives.
	// Further, since we had to store registers above without clobbering them,
	// we had to do this afterward since there weren't two un-clobbered registers
	// to use.
	asm! {
		"mov rax, rbp",
		"mov rsi, rbx",
		out("rax") frame.rbp,
		out("rsi") frame.rbx,
		options(nostack),
	};

	// Now we get the address of the kernel stack from the core local
	// state.
	let kernel_stack = crate::Kernel::get().handle().kernel_stack.get().read();

	// We then set the new stack pointer, subtracting the size of the
	// frame from the kernel stack pointer, and copying the frame to
	// the new stack.
	asm! {
		"mov rsp, rax",
		"jmp syscall_enter_non_compat_stage3",
		in("r8") core::ptr::from_ref(&frame),
		in("rax") kernel_stack,
		options(noreturn),
	};
}

/// The third stage of the 64-bit, non-compatibility-mode syscall entry.
#[no_mangle]
unsafe extern "C" fn syscall_enter_non_compat_stage3() -> ! {
	// Now we can copy the ABI frame to the new stack.
	let abi_frame_ptr: u64;

	asm! {
		"",
		out("r8") abi_frame_ptr,
	};

	let mut abi_frame = (abi_frame_ptr as *const AbiCallFrame).read_volatile();

	// Sanity check that the syscall instruction cleared the IF flag.
	debug_assert!(
		!crate::asm::interrupts_enabled(),
		"syscall instruction did not clear IF"
	);

	// Now we can call the syscall handler.
	// XXX(qix-): placeholder stub
	abi_frame.rax = oro_sysabi::syscall::Error::NotImplemented as u64;
	abi_frame.rdx = 0;
	return_to_user_from_syscall(abi_frame)
}

/// Returns to userspace from a syscall (previously constructed from the
/// `syscall_enter_non_compat` entry point).
///
/// # Safety
/// **This is an incredibly sensitive security boundary.** Calls to this
/// function should be done with extreme caution and care.
///
/// Caller must ensure that the task is ready to be executed again. This means
/// the memory map, core-local mappings, etc. are all restored to the task that
/// originally made the syscall that created the given [`AbiCallFrame`].
///
/// **Interrupts must be disabled when calling this function.**
unsafe fn return_to_user_from_syscall(frame: AbiCallFrame) -> ! {
	// TODO(qix-): There is almost definitely some missing functionality here, namely
	// TODO(qix-): around the resume flag (RF) and the trap flag (TF) in the RFLAGS register.

	// Restore the frame.
	asm! {
		// Force the return address to a canonical address.
		"and rcx, CANONICAL_ADDRESS_MASK",
		// Restore the stack.
		"mov rsp, r9",
		// Restore RBP/RBX.
		"mov rbp, r10",
		"mov rbx, r8",
		// Zero clobbered registers.
		//
		// SAFETY(qix-): Vector registers are clobbered AND considered insecurely transferred.
		// SAFETY(qix-): It is specified that the kernel DOES NOT zero vector registers.
		"xor r8, r8",
		"xor r9, r9",
		"xor r10, r10",
		"xor r11, r11",
		"xor rdi, rdi",
		"xor rsi, rsi",
		// Return to userspace.
		"sysretq",
		in("rax") frame.rax,
		in("rdx") frame.rdx,
		in("r9") frame.rsp,
		in("r12") frame.r12,
		in("r13") frame.r13,
		in("r14") frame.r14,
		in("r15") frame.r15,
		in("rcx") frame.rcx,
		in("r11") frame.rflags,
		in("r8") frame.rbx,
		in("r10") frame.rbp,
		options(noreturn),
	}
}

#[doc(hidden)]
const fn must_be_u16(x: u16) -> u16 {
	x
}

impl oro_kernel::arch::SystemCallHandle for AbiCallFrame {
	#[inline]
	fn opcode(&self) -> oro_sysabi::syscall::Opcode {
		// SAFETY: This is safe because the opcode is non_exhaustive and the
		// SAFETY: kernel validates the value before using it.
		unsafe { core::mem::transmute(self.rax) }
	}

	#[inline]
	fn table_id(&self) -> u64 {
		self.rsi
	}

	#[inline]
	fn key(&self) -> u64 {
		self.rdi
	}

	#[inline]
	fn value(&self) -> u64 {
		self.rdx
	}

	#[inline]
	fn entity_id(&self) -> u64 {
		self.r9
	}

	#[inline]
	fn set_error(&mut self, error: oro_sysabi::syscall::Error) {
		self.rax = error as u64;
	}

	#[inline]
	fn set_return_value(&mut self, value: u64) {
		self.rdx = value;
	}

	#[inline]
	unsafe fn return_to_caller(self) -> ! {
		return_to_user_from_syscall(self)
	}
}
