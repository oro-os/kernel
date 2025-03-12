//! Core local kernel handle for the x86_64 architecture.

use core::{cell::UnsafeCell, mem::MaybeUninit};

use oro_kernel::{arch::Arch, event::Resumption};

use crate::{
	gdt, lapic,
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
	tss,
};

/// Core local kernel handle for the x86_64 architecture.
///
/// Used primarily to issue interrupts and syscalls.
pub struct CoreHandle {
	/// The LAPIC (Local Advanced Programmable Interrupt Controller)
	/// for the core.
	pub lapic: lapic::Lapic,
	/// The core's local GDT
	///
	/// Only valid after the Kernel has been initialized
	/// and properly mapped.
	pub gdt:   UnsafeCell<MaybeUninit<gdt::Gdt<8>>>,
	/// The TSS (Task State Segment) for the core.
	pub tss:   UnsafeCell<tss::Tss>,
}

unsafe impl oro_kernel::arch::CoreHandle<crate::Arch> for CoreHandle {
	fn schedule_timer(&self, ticks: u32) {
		self.lapic.set_timer_initial_count(ticks);
	}

	fn cancel_timer(&self) {
		self.lapic.cancel_timer();
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
