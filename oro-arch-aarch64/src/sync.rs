//! Provides synchronization-related types, traits and implementations.

use core::arch::asm;

/// Implements interupt controlling for unfair critical locks.
pub struct InterruptController;

impl oro_sync::spinlock::unfair_critical::InterruptController for InterruptController {
	type InterruptState = usize;

	fn disable_interrupts() {
		crate::asm::disable_interrupts();
	}

	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("mrs {}, daif", out(reg) flags, options(nostack, nomem));
		}
		flags
	}

	fn restore_interrupts(state: Self::InterruptState) {
		unsafe {
			asm!("msr daif, {}", in(reg) state, options(nostack, nomem));
		}
	}
}
