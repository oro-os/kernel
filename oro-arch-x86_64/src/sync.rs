//! Provides synchronization-related types, traits and implementations.

use core::arch::asm;

/// Implements interrupt controlling for unfair critical locks.
pub struct InterruptController;

impl oro_sync::spinlock::unfair_critical::InterruptController for InterruptController {
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
