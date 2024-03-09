use core::arch::asm;

/// Invalidates a single page in the Translation Lookaside Buffer (TLB)
/// given a `virtual_address`.
pub unsafe fn invlpg(virtual_address: u64) {
	asm!(
		"invlpg [{}]",
		in(reg) virtual_address,
		options(nostack, preserves_flags)
	);
}
