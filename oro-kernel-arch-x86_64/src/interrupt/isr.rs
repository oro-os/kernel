//! Default interrupt ISRs.

use core::{arch::global_asm, cell::UnsafeCell};

use oro_arch_x86_64::paging::PagingLevel;

use crate::{
	interrupt::{InvalidInstruction, PageFault, PageFaultAccess, PreemptionEvent, StackFrame},
	mem::address_space::AddressSpaceLayout,
};

/// Common entry point for the ISR handlers.
///
/// This function is passed a pointer to the base
/// of the `StackFrame` on the user thread's shadow
/// stack - or, in the event the interrupt came from
/// within the kernel itself, on the kernel's stack.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_handler(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	// SAFETY: This entire thing is inherently unsafe; there's no point
	// SAFETY: in skirting around it, we're glueing low-level CPU code
	// SAFETY: to a higher level Rust kernel.
	unsafe {
		// Make sure the `as` casts won't truncate.
		oro_kernel_macro::assert::fits_within::<u64, usize>();

		debug_assert!(stack_ptr.is_aligned());
		let fp = &*stack_ptr;

		debug_assert!(
			((*fp.get()).cs & 3) == 3 || (*fp.get()).iv >= 32,
			"_oro_isr_rust_handler called with kernel exception (core panic handler wasn't called)"
		);

		let kernel = crate::Kernel::get();
		let iv = (*fp.get()).iv;

		if iv >= 32 {
			// Tell the PIC to de-assert.
			kernel.handle().lapic.eoi();
		}

		let preemption_event = match iv {
			// Invalid opcode.
			0x06 => {
				PreemptionEvent::InvalidInstruction(InvalidInstruction {
					ip: (*fp.get()).ip as usize,
				})
			}
			// Page fault.
			0x0E => {
				PreemptionEvent::PageFault(PageFault {
					address: oro_arch_x86_64::cr2() as usize,
					ip:      Some((*fp.get()).ip as usize),
					access:  {
						let err = (*fp.get()).err;
						if (err & 0b0001_0000) != 0 {
							PageFaultAccess::Execute
						} else if err & 0b0000_0010 != 0 {
							PageFaultAccess::Write
						} else {
							PageFaultAccess::Read
						}
					},
				})
			}
			// Timer
			0x20 => PreemptionEvent::Timer,
			// Unhandled exception?
			// XXX(qix-): This is temporary
			iv if iv < 32 => {
				todo!(
					"unhandled userspace exception: {:#016X?}",
					&*(*stack_ptr).get()
				);
			}
			iv => PreemptionEvent::Interrupt(iv),
		};

		kernel.handle_event(preemption_event);
	}
}

/// Debug assertion function that is called by the
/// interrupt stubs if the stack is unaligned.
#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_dbg_stack_unaligned(
	got: u64,
	alignment: u64,
	stack_ptr: u64,
	expected: u64,
	rip: u64,
) -> ! {
	use oro_debug::dbg_err;

	dbg_err!(
		"ISR STACK MISALIGNED: modulo={got:#016X}, expected={expected:#016X}, \
		 align={alignment:#016X}, rsp={stack_ptr:#016X}, rip={rip:#016X}"
	);

	if (stack_ptr & 7) != 0 {
		dbg_err!("stack pointer is NOT 64-bit aligned; below values will be garbage");
	}

	// Try to figure out which segment the stack belongs to.
	let pl = PagingLevel::current_from_cpu();
	let top_level_idx = match pl {
		PagingLevel::Level4 => (stack_ptr >> (12 + 9 * 3)) & 0x1FF,
		PagingLevel::Level5 => (stack_ptr >> (12 + 9 * 4)) & 0x1FF,
	};

	match top_level_idx as usize {
		AddressSpaceLayout::MODULE_INTERRUPT_STACK_IDX => {
			dbg_err!("RSP is in MODULE_INTERRUPT_STACK_IDX");
		}
		AddressSpaceLayout::MODULE_THREAD_STACK_IDX => {
			dbg_err!("RSP is in MODULE_THREAD_STACK_IDX");
		}
		AddressSpaceLayout::KERNEL_STACK_IDX => {
			dbg_err!("RSP is in KERNEL_STACK_IDX");
		}
		unknown => {
			dbg_err!("RSP is in UNKNOWN STACK INDEX ({unknown}, pl={pl:?})");
		}
	}

	let end = match pl {
		PagingLevel::Level4 => {
			crate::sign_extend!(L4, (top_level_idx << (12 + 9 * 3)) | 0x0000_007F_FFFF_F000)
		}
		PagingLevel::Level5 => {
			crate::sign_extend!(L5, (top_level_idx << (12 + 9 * 4)) | 0x0000_FFFF_FFFF_F000)
		}
	} as u64;
	let start = stack_ptr & !7;

	// SAFETY: Doesn't really matter, this is debugging best-effort, as this
	// SAFETY: is a case of a bug in the kernel.
	unsafe {
		let slice = UnsafeCell::new(::core::slice::from_raw_parts(
			start as *const u64,
			((end - start) >> 3) as usize,
		));
		let slice_ref = &*slice.get();

		dbg_err!("    BEGIN STACK");
		for (i, v) in slice_ref.iter().rev().enumerate() {
			dbg_err!("    {:016X}: {v:016X}", i * 8);
		}
		dbg_err!("    END STACK");
	}

	panic!("ISR stack misaligned")
}

/// Core panic.
#[unsafe(no_mangle)]
extern "C" fn _oro_isr_rust_core_panic(stack_ptr: *const UnsafeCell<StackFrame>) -> ! {
	let _ = stack_ptr; // NOTE(qix-): Marked as unused on release modes.

	#[cfg(debug_assertions)]
	{
		use core::fmt::Write;

		const HEX: &[u8] = b"0123456789ABCDEF";

		macro_rules! log_hex {
			($v:expr) => {
				let b = [
					HEX[(($v >> 60) & 0xF) as usize],
					HEX[(($v >> 56) & 0xF) as usize],
					HEX[(($v >> 52) & 0xF) as usize],
					HEX[(($v >> 48) & 0xF) as usize],
					HEX[(($v >> 44) & 0xF) as usize],
					HEX[(($v >> 40) & 0xF) as usize],
					HEX[(($v >> 36) & 0xF) as usize],
					HEX[(($v >> 32) & 0xF) as usize],
					HEX[(($v >> 28) & 0xF) as usize],
					HEX[(($v >> 24) & 0xF) as usize],
					HEX[(($v >> 20) & 0xF) as usize],
					HEX[(($v >> 16) & 0xF) as usize],
					HEX[(($v >> 12) & 0xF) as usize],
					HEX[(($v >> 8) & 0xF) as usize],
					HEX[(($v >> 4) & 0xF) as usize],
					HEX[($v & 0xF) as usize],
				];

				// SAFETY: We know the string is valid UTF-8.
				let _ =
					oro_debug::DebugWriter.write_str(unsafe { core::str::from_utf8_unchecked(&b) });
			};
		}

		oro_debug::dbg_err!("unhandled exception; core is about to panic");
		// SAFETY: We have to assume it's valid.
		let fr = unsafe { &*(*stack_ptr).get() };

		macro_rules! log_field {
			($label:literal, $f:ident) => {
				let _ = oro_debug::DebugWriter.write_str(concat!("\n", $label, ":\t"));
				log_hex!(fr.$f);
			};
		}

		macro_rules! log_var {
			($label:literal, $v:expr) => {
				let _ = oro_debug::DebugWriter.write_str(concat!("\n", $label, ":\t"));
				log_hex!($v);
			};
		}

		let cr0: u64 = oro_arch_x86_64::reg::Cr0::load().into();
		let cr2: u64 = oro_arch_x86_64::cr2();
		let cr3: u64 = oro_arch_x86_64::cr3();
		let cr4: u64 = oro_arch_x86_64::reg::Cr4::load().into();
		let lapic_id_u8 = oro_arch_x86_64::cpuid::CpuidA01C0::get().map(|v| v.ebx.local_apic_id());

		log_field!("IV", iv);
		log_field!("IP", ip);
		log_field!("SP", sp);
		log_field!("CS", cs);
		log_field!("SS", ss);
		log_field!("ERR", err);
		log_field!("FLAGS", flags);
		log_var!("CR0", cr0);
		log_var!("CR2", cr2);
		log_var!("CR3", cr3);
		log_var!("CR4", cr4);
		log_field!("RAX", rax);
		log_field!("RBX", rbx);
		log_field!("RCX", rcx);
		log_field!("RDX", rdx);
		log_field!("RSI", rsi);
		log_field!("RDI", rdi);
		log_field!("RBP", rbp);
		log_field!("R8", r8);
		log_field!("R9", r9);
		log_field!("R10", r10);
		log_field!("R11", r11);
		log_field!("R12", r12);
		log_field!("R13", r13);
		log_field!("R14", r14);
		log_field!("R15", r15);
		if let Some(lapic_id_u8) = lapic_id_u8 {
			let lapic_id = u64::from(lapic_id_u8);
			log_var!("LAPIC ID (<=255)", lapic_id);
		} else {
			let _ = oro_debug::DebugWriter.write_str("\nLAPIC ID (<=255):\t(unknown)");
		}

		let _ = oro_debug::DebugWriter.write_str("\n\nEND OF CORE DUMP\n");
	}

	// SAFETY: Not much we can do here anyway.
	panic!("core panicked");
}

/// Performs an `iret` into userspace code.
///
/// This function **does** modify the local core's
/// TSS pointers to point to the stack frame base
/// on DPL=3 -> DPL=0 code.
///
/// # Safety
/// The given task context MUST be ready for a context switch,
/// must NOT be run anywhere else, and the CPU must be ready
/// to receive interrupts (kernel initialized, IDT installed, etc).
///
/// This function **may not** be used to switch into kernel (ring 0)
/// code.
///
/// **All locks or other stack-based stateful objects must be destroyed
/// prior to this function being called.** The kernel is entirely
/// destroyed when this function is called.
#[inline]
pub unsafe fn iret_context(cr3: u64) -> ! {
	unsafe extern "C" {
		#[link_name = "_oro_isr_iret"]
		fn oro_isr_iret(cr3: u64, irq_frame_base: u64) -> !;
	}

	let irq_stack_base = AddressSpaceLayout::irq_stack_base(PagingLevel::current_from_cpu()) as u64;

	// SAFETY: We can guarantee that we're the only users of this handle
	// SAFETY: given that `Kernel` handles are core-local.
	unsafe {
		(*crate::Kernel::get().handle().tss.get())
			.rsp0
			.write(irq_stack_base);
	}

	let irq_frame_base = irq_stack_base - size_of::<StackFrame>() as u64;

	oro_isr_iret(cr3, irq_frame_base)
}

#[doc(hidden)]
#[cfg(debug_assertions)]
macro_rules! define_all_handlers {
	() => {
		"DEFINE_ALL_HANDLERS CHECK_STACK_ALIGNMENT_DEBUG"
	};
}
#[doc(hidden)]
#[cfg(not(debug_assertions))]
macro_rules! define_all_handlers {
	() => {
		"DEFINE_ALL_HANDLERS CHECK_STACK_ALIGNMENT_NOOP"
	};
}

global_asm! {
	include_str!("../common-pre.S"),
	include_str!("./isr64.S"),
	define_all_handlers!(),
	include_str!("../common-post.S"),
	CS_OFFSET = const core::mem::offset_of!(StackFrame, cs),
	KERNEL_STACK_BASE_L4 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level4),
	KERNEL_STACK_BASE_L5 = const AddressSpaceLayout::kernel_stack_base(PagingLevel::Level5),
}
