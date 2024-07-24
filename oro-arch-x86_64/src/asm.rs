//! Assembly instruction stubs for the x86_64 architecture.

#![allow(clippy::inline_always)]

use core::arch::asm;

/// Invalidates a single page in the Translation Lookaside Buffer (TLB)
/// given a `virtual_address`.
#[inline(always)]
pub fn invlpg(virtual_address: usize) {
	unsafe {
		asm!(
			"invlpg [{}]",
			in(reg) virtual_address,
			options(nostack, preserves_flags)
		);
	}
}

/// Returns whether or not 5-level paging is enabled.
#[inline(always)]
#[must_use]
pub fn is_5_level_paging_enabled() -> bool {
	let cr4: usize;
	unsafe {
		asm!("mov {}, cr4", out(reg) cr4, options(preserves_flags));
	}
	cr4 & (1 << 12) != 0
}

/// Returns the current value of the `cr3` register
#[inline(always)]
#[must_use]
pub fn cr3() -> u64 {
	let cr3: u64;
	unsafe {
		asm!("mov {}, cr3", out(reg) cr3, options(preserves_flags));
	}
	cr3
}

/// Sets the value of the `cr3` register to `value`.
///
/// # Safety
/// Callers must be prepared for the consequences of changing the
/// page table base address.
#[inline(always)]
pub unsafe fn _set_cr3(value: u64) {
	asm!("mov cr3, {}", in(reg) value, options(nostack, preserves_flags));
}
