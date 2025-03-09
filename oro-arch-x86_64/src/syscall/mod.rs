//! Syscall handler implementation for x86_64.

use core::{arch::global_asm, cell::UnsafeCell};

use oro_kernel::event::{PreemptionEvent, SystemCallRequest};

use crate::{
	asm::{rdmsr, wrmsr},
	interrupt::StackFrame,
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
};

/// Caches the system's paging level.
#[unsafe(no_mangle)]
static mut ORO_SYSCALL_CANONICAL_ADDRESS_MASK: u64 = !0;

/// The base address of a user task's IRQ stack.
#[unsafe(no_mangle)]
static mut ORO_SYSCALL_IRQ_STACK_BASE: usize = 0;

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

	// Tell the CPU to clear the interrupt enable flag (IF), trap flag (TF),
	// and direction flag (DF) when executing a syscall via the SFMASK MSR.
	//
	// TODO(qix-): Make an RFLAGS register in crate::reg and use that instead
	// TODO(qix-): of hardcoding the value.
	wrmsr(0xC000_0084, 0x0200 | 0x0100 | 0x0400);

	// Tell the CPU which CS and SS selectors to use when executing a syscall.
	// See the `STAR` constant in the `gid` module for more information.
	#[doc(hidden)]
	const STAR: u64 = ((must_be_u16(crate::gdt::STAR_KERNEL) as u64) << 32)
		| ((must_be_u16(crate::gdt::STAR_USER) as u64) << 48);

	wrmsr(0xC000_0081, STAR);

	unsafe extern "C" {
		#[link_name = "_oro_syscall_enter"]
		fn oro_syscall_enter() -> !;
	}

	// Tell the CPU where to jump to when a syscall is executed from x86_64 mode (LSTAR MSR).
	wrmsr(0xC000_0082, oro_syscall_enter as *const () as u64);

	// Get the paging level and construct a canonical address mask.
	ORO_SYSCALL_CANONICAL_ADDRESS_MASK = match PagingLevel::current_from_cpu() {
		// 47-bit canonical addresses (lower-half 48 bit address)
		PagingLevel::Level4 => 0x0000_7FFF_FFFF_FFFF,
		// 56-bit canonical addresses (lower-half 57 bit address)
		PagingLevel::Level5 => 0x00FF_FFFF_FFFF_FFFF,
	};

	// Get the base address of a user task's IRQ stack.
	ORO_SYSCALL_IRQ_STACK_BASE =
		AddressSpaceLayout::irq_stack_base(PagingLevel::current_from_cpu());
}

/// Called by the syscall assembly stubs (sysenter) after a partial [`StackFrame`]
/// has been populated.
///
/// The `StackFrame` might have "stale" data in it but is suitable for either a
/// `sysret` or an `iret` return to the user task as per the Oro ABI specification.
#[unsafe(no_mangle)]
extern "C" fn _oro_syscall_handler(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	debug_assert!(stack_ptr.is_aligned());

	// SAFETY: We have to assume this is safe; it's passed in directly
	// SAFETY: by the ASM stubs.
	let fp = unsafe { &*stack_ptr };

	// SAFETY: Same safety consideration as above.
	let syscall_request = unsafe {
		SystemCallRequest {
			opcode: (*fp.get()).rax,
			arg1:   (*fp.get()).rsi,
			arg2:   (*fp.get()).rdi,
			arg3:   (*fp.get()).rdx,
			arg4:   (*fp.get()).r9,
		}
	};

	// Fire it off to the kernel.
	// SAFETY: We can assume that since we're coming from a syscall that it was the
	// SAFETY: thread that was originally scheduled by this core. We have no other way
	// SAFETY: of verifying that here.
	unsafe {
		crate::Kernel::get().handle_event(PreemptionEvent::SystemCall(syscall_request));
	}
}

#[doc(hidden)]
#[cfg(debug_assertions)]
macro_rules! define_syscall_handlers {
	() => {
		"DEFINE_SYSCALL_HANDLERS CHECK_STACK_ALIGNMENT_DEBUG"
	};
}
#[doc(hidden)]
#[cfg(not(debug_assertions))]
macro_rules! define_syscall_handlers {
	() => {
		"DEFINE_SYSCALL_HANDLERS CHECK_STACK_ALIGNMENT_NOOP"
	};
}

global_asm! {
	include_str!("../common-pre.S"),
	include_str!("./syscall.S"),
	define_syscall_handlers!(),
	include_str!("../common-post.S"),
	STACK_FRAME_SIZE = const core::mem::size_of::<StackFrame>(),
	USER_CS = const crate::gdt::USER_CS | 3,
	USER_SS = const crate::gdt::USER_DS | 3,
	CS_OFFSET = const core::mem::offset_of!(StackFrame, cs),
	SS_OFFSET = const core::mem::offset_of!(StackFrame, ss),
	SP_OFFSET = const core::mem::offset_of!(StackFrame, sp),
	R15_OFFSET = const core::mem::offset_of!(StackFrame, r15),
	R14_OFFSET = const core::mem::offset_of!(StackFrame, r14),
	R13_OFFSET = const core::mem::offset_of!(StackFrame, r13),
	R12_OFFSET = const core::mem::offset_of!(StackFrame, r12),
	RAX_OFFSET = const core::mem::offset_of!(StackFrame, rax),
	RSI_OFFSET = const core::mem::offset_of!(StackFrame, rsi),
	RDI_OFFSET = const core::mem::offset_of!(StackFrame, rdi),
	RDX_OFFSET = const core::mem::offset_of!(StackFrame, rdx),
	R9_OFFSET = const core::mem::offset_of!(StackFrame, r9),
	RBP_OFFSET = const core::mem::offset_of!(StackFrame, rbp),
	RBX_OFFSET = const core::mem::offset_of!(StackFrame, rbx),
	FLAGS_OFFSET = const core::mem::offset_of!(StackFrame, flags),
	IP_OFFSET = const core::mem::offset_of!(StackFrame, ip),
	IV_OFFSET = const core::mem::offset_of!(StackFrame, iv),
	KERNEL_STACK_BASE_L4 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level4),
	KERNEL_STACK_BASE_L5 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level5),
}

#[doc(hidden)]
const fn must_be_u16(v: u16) -> u16 {
	v
}
