//! Main [`Arch`] implementation for the Aarch64 architecture.

#![allow(clippy::inline_always, clippy::verbose_bit_mask)]

use crate::mem::address_space::AddressSpaceLayout;
use core::arch::asm;
use oro_common::arch::Arch;
use oro_common_elf::{ElfClass, ElfEndianness, ElfMachine};

/// AArch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

unsafe impl Arch for Aarch64 {
	type AddressSpace = AddressSpaceLayout;
	type InterruptState = usize;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

	fn halt_once_and_wait() {
		unsafe {
			asm!("wfi");
		}
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("dsb sy", options(nostack, preserves_flags),);
		}
	}
}

impl oro_common_sync::spinlock::unfair_critical::InterruptController for Aarch64 {
	type InterruptState = <Self as Arch>::InterruptState;

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
