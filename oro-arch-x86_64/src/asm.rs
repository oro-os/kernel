#![allow(clippy::inline_always)]

use core::arch::asm;

/// Invalidates a single page in the Translation Lookaside Buffer (TLB)
/// given a `virtual_address`.
#[inline(always)]
pub fn invlpg(virtual_address: u64) {
	unsafe {
		asm!(
			"invlpg [{}]",
			in(reg) virtual_address,
			options(nostack, preserves_flags)
		);
	}
}
