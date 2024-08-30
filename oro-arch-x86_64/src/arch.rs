//! Implements the [`Arch`] architecture trait for the x86_64 architecture.

#![allow(clippy::inline_always)]

use core::arch::asm;
use oro_common::arch::Arch;
use oro_common_elf::{ElfClass, ElfEndianness, ElfMachine};

/// x86_64 architecture support implementation for the Oro kernel.
pub struct X86_64;

unsafe impl Arch for X86_64 {
	type AddressSpace = crate::mem::address_space::AddressSpaceLayout;
	type InterruptState = usize;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::X86_64;

	fn halt_once_and_wait() {
		unsafe {
			asm!("cli", "hlt");
		}
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
	}
}

impl oro_common_sync::spinlock::unfair_critical::InterruptController for X86_64 {
	type InterruptState = <Self as Arch>::InterruptState;

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
