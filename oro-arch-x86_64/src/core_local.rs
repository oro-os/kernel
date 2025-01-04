//! Core local kernel handle for the x86_64 architecture.

use core::{cell::UnsafeCell, mem::MaybeUninit};

use crate::{gdt, lapic, tss};

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
	pub gdt: UnsafeCell<MaybeUninit<gdt::Gdt<8>>>,
	/// The TSS (Task State Segment) for the core.
	pub tss: UnsafeCell<tss::Tss>,
	/// The kernel's stored stack pointer.
	pub kernel_stack: UnsafeCell<u64>,
	/// The IRQ head of the kernel stack (with GP registers)
	// TODO(qix-): This is probably unnecessary, and was only included
	// TODO(qix-): as a precaution against stack leak when switching to/from
	// TODO(qix-): the kernel task. This should be removed if it's not needed.
	pub kernel_irq_stack: UnsafeCell<u64>,
}

unsafe impl oro_kernel::arch::CoreHandle for CoreHandle {
	fn schedule_timer(&self, ticks: u32) {
		self.lapic.set_timer_initial_count(ticks);
	}

	fn cancel_timer(&self) {
		self.lapic.cancel_timer();
	}
}
