//! Syscall handler implementation for x86_64.

use core::arch::{asm, naked_asm};

use oro_kernel::scheduler::{Switch, SystemCallRequest};
use oro_mem::mapper::AddressSegment;
use oro_sync::Lock;
use oro_sysabi::syscall::Opcode;

use crate::{
	asm::{rdmsr, wrmsr},
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
};

/// Caches the system's paging level.
#[no_mangle]
static mut ORO_SYSCALL_CANONICAL_ADDRESS_MASK: u64 = !0;

/// The base address of a user task's IRQ stack.
#[no_mangle]
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

	// Tell the CPU where to jump to when a syscall is executed from x86_64 mode (LSTAR MSR).
	wrmsr(0xC000_0082, syscall_enter_noncompat as *const () as u64);

	// Get the paging level and construct a canonical address mask.
	ORO_SYSCALL_CANONICAL_ADDRESS_MASK = match PagingLevel::current_from_cpu() {
		// 47-bit canonical addresses (lower-half 48 bit address)
		PagingLevel::Level4 => 0x0000_7FFF_FFFF_FFFF,
		// 56-bit canonical addresses (lower-half 57 bit address)
		PagingLevel::Level5 => 0x00FF_FFFF_FFFF_FFFF,
	};

	// Get the base address of a user task's IRQ stack.
	ORO_SYSCALL_IRQ_STACK_BASE = AddressSpaceLayout::interrupt_stack().range().1 & !0xFFF;
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
unsafe extern "C" fn syscall_enter_noncompat() -> ! {
	crate::syscall_store_task_and_jmp!(syscall_enter_noncompat_rust)
}

/// Rust entry point for 64-bit, non-compatibility-mode syscalls.
///
/// # Safety
/// **This is an incredibly sensitive security boundary.**
/// Maintenance on this function should be done with extreme caution and care.
///
/// Not to be called directly by kernel code.
#[inline(never)]
#[no_mangle]
unsafe extern "C" fn syscall_enter_noncompat_rust() -> ! {
	let stack_ptr: usize;
	let opcode: u64;
	let arg1: u64;
	let arg2: u64;
	let arg3: u64;
	let arg4: u64;
	asm! {
		"",
		out("r8") stack_ptr,
		out("rax") opcode,
		out("rsi") arg1,
		out("rdi") arg2,
		out("rdx") arg3,
		out("r9") arg4,
	};

	let opcode = core::mem::transmute::<u64, Opcode>(opcode);

	let syscall_request = SystemCallRequest {
		opcode,
		arg1,
		arg2,
		arg3,
		arg4,
	};

	let mut scheduler = crate::Kernel::get().scheduler().lock();
	let Some(current_thread) = scheduler.current_thread() else {
		// SAFETY(qix-): This is a bug if we reach here. There should
		// SAFETY(qix-): always be a current thread, as there's no other
		// SAFETY(qix-): way a syscall could have fired (the Oro kernel
		// SAFETY(qix-): does not support `syscall` from kernel contexts).
		unreachable!();
	};

	current_thread.lock().handle_mut().irq_stack_ptr = stack_ptr;

	let switch = scheduler.event_system_call(&syscall_request);

	drop(scheduler);

	match switch {
		Switch::KernelResume | Switch::KernelToUser(_, _) => {
			// The Oro kernel cannot invoke syscalls. This should never happen.
			unreachable!();
		}
		Switch::UserResume(user_ctx, None) | Switch::UserToUser(user_ctx, None) => {
			let kernel = crate::Kernel::get();

			let (thread_cr3_phys, thread_rsp) = unsafe {
				let ctx_lock = user_ctx.lock();

				let mapper = ctx_lock.mapper();
				let cr3 = mapper.base_phys;
				let rsp = ctx_lock.handle().irq_stack_ptr;
				(*kernel.handle().tss.get())
					.rsp0
					.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
				drop(ctx_lock);
				(cr3, rsp)
			};

			asm! {
				"jmp oro_x86_64_user_to_user",
				in("rax") thread_cr3_phys,
				in("rdx") thread_rsp,
				options(noreturn),
			}
		}
		Switch::UserResume(user_ctx, Some(syscall_response))
		| Switch::UserToUser(user_ctx, Some(syscall_response)) => {
			let kernel = crate::Kernel::get();

			let (thread_cr3_phys, thread_rsp) = unsafe {
				let ctx_lock = user_ctx.lock();

				let mapper = ctx_lock.mapper();
				let cr3 = mapper.base_phys;
				let rsp = ctx_lock.handle().irq_stack_ptr;
				(*kernel.handle().tss.get())
					.rsp0
					.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
				drop(ctx_lock);
				(cr3, rsp)
			};

			asm! {
				"jmp oro_x86_64_user_to_user_sysret",
				in("r8") thread_cr3_phys,
				in("r10") thread_rsp,
				in("rax") syscall_response.error as u64,
				in("r9") syscall_response.ret,
				options(noreturn)
			}
		}
		Switch::UserToKernel => {
			let kernel = crate::Kernel::get();
			let kernel_irq_stack = kernel.handle().kernel_irq_stack.get().read();
			let kernel_stack = kernel.handle().kernel_stack.get().read();
			let kernel_cr3 = kernel.mapper().base_phys;

			asm! {
				"mov cr3, rdx",
				"mov rsp, rcx",
				"jmp oro_x86_64_return_to_kernel",
				in("rcx") kernel_irq_stack,
				in("r9") kernel_stack,
				in("rdx") kernel_cr3,
				options(noreturn),
			}
		}
	}
}

#[doc(hidden)]
const fn must_be_u16(x: u16) -> u16 {
	x
}
