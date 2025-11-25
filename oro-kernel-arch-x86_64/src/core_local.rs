//! Core local kernel handle for the x86_64 architecture.

use core::{cell::UnsafeCell, mem::MaybeUninit};

use oro_arch_x86_64::{gdt::Gdt, lapic::Lapic, paging::PagingLevel, tss::Tss};
use oro_kernel::{
	arch::{Arch, InstantResult},
	event::Resumption,
};
use oro_kernel_mem::alloc::{boxed::Box, sync::Arc};

use crate::{interrupt::Idt, mem::address_space::AddressSpaceLayout, time::GetInstant};

/// Core local kernel handle for the x86_64 architecture.
///
/// Used primarily to issue interrupts and syscalls.
pub struct CoreHandle {
	/// The LAPIC (Local Advanced Programmable Interrupt Controller)
	/// for the core.
	pub lapic:       Lapic,
	/// The core's local GDT
	///
	/// Only valid after the Kernel has been initialized
	/// and properly mapped.
	pub gdt:         UnsafeCell<MaybeUninit<Gdt<8>>>,
	/// The TSS (Task State Segment) for the core.
	pub tss:         UnsafeCell<Tss>,
	/// The core local IDT.
	pub idt:         Box<Idt>,
	/// The implementation of the timestamp fetcher.
	pub instant_gen: Arc<dyn GetInstant>,
}

unsafe impl oro_kernel::arch::CoreHandle<crate::Arch> for CoreHandle {
	type Instant = crate::time::Instant;

	fn schedule_timer(&self, ticks: u32) {
		self.lapic.set_timer_initial_count(ticks);
	}

	fn cancel_timer(&self) {
		self.lapic.cancel_timer();
	}

	fn now(&self) -> InstantResult<Self::Instant> {
		InstantResult::Ok(self.instant_gen.now())
	}

	unsafe fn run_context(
		&self,
		context: Option<&UnsafeCell<<crate::Arch as Arch>::ThreadHandle>>,
		ticks: Option<u32>,
		resumption: Option<Resumption>,
	) -> ! {
		if let Some(context) = context {
			if let Some(ticks) = ticks {
				self.schedule_timer(ticks);
			}

			match &resumption {
				None => (*context.get()).iret(),
				Some(Resumption::SystemCall(res)) => (*context.get()).sysret(res),
			}
		} else {
			// Go to sleep.
			let kernel_stack_base =
				AddressSpaceLayout::kernel_stack_base(PagingLevel::current_from_cpu());

			if let Some(ticks) = ticks {
				self.schedule_timer(ticks);
			}

			::core::arch::asm! {
				// Clear the stack pointer
				"mov rsp, {}",
				// Enable interrupts
				"sti",
				// Halt once
				"hlt",
				"ud2",
				in(reg) kernel_stack_base,
				options(noreturn),
			}
		}
	}
}
