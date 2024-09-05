//! Implements the [`Arch`] architecture trait for the x86_64 architecture.

#![allow(clippy::inline_always)]

use core::arch::asm;

/// x86_64 architecture support implementation for the Oro kernel.
pub struct X86_64;

impl X86_64 {
	/// Halts, indefinitely, the CPU (disabling interrupts).
	pub fn halt() -> ! {
		unsafe {
			asm!("cli");
		}
		loop {
			Self::halt_once_and_wait();
		}
	}

	/// Halts the CPU once and waits for an interrupt.
	pub fn halt_once_and_wait() {
		unsafe {
			asm!("hlt");
		}
	}

	/// Performs a strong memory serialization barrier.
	#[inline(always)]
	pub fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
	}
}

impl oro_sync::spinlock::unfair_critical::InterruptController for X86_64 {
	type InterruptState = usize;

	fn disable_interrupts() {
		crate::asm::disable_interrupts();
	}

	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("pushfq", "pop {}", out(reg) flags, options(nostack));
		}
		flags
	}

	fn restore_interrupts(state: Self::InterruptState) {
		unsafe {
			asm!("push {}", "popfq", in(reg) state, options(nostack));
		}
	}
}
