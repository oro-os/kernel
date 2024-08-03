//! One-off assembly instructions or operations for AArch64.
use core::arch::asm;

/// Invalidates the TLB entry for the given virtual address.
///
/// # Safety
/// Caller must ensure that the virtual address is valid and aligned.
pub unsafe fn invalidate_tlb_el1(virt: usize) {
	asm!(
		"dsb ish",                // Ensure the update is visible
		"dc ivac, {0:x}",         // Invalidate the data cache by virtual address
		"ic ivau, {0:x}",         // Invalidate the instruction cache by virtual address
		"tlbi vaae1, {0}",        // Invalidate the TLB entry by virtual address for EL1
		"dsb ish",                // Ensure completion of the invalidation
		"isb",                    // Synchronize the instruction stream
		in(reg) virt,
		options(nostack, preserves_flags),
	);
}

/// Loads the current `TTBR0_EL1` register value.
///
/// # Safety
/// Caller must ensure that any references derived
/// from this function do not violate Rust's
/// aliasing (multiple-reference) rules, namely
/// when creating mutable references.
///
/// Further, depending on the execution context,
/// the page tables referred to by this register
/// may be entirely undefined.
///
/// This value is NOT guaranteed to be unique across
/// all cores in all execution contexts. Be VERY CAREFUL
/// that you control the value in the `TTBR0_EL1` register
/// prior to calling this again.
#[allow(dead_code)]
pub unsafe fn load_ttbr0() -> u64 {
	let ttbr0_el1: u64;
	unsafe {
		asm!(
			"mrs {0:x}, TTBR0_EL1",
			out(reg) ttbr0_el1
		);
	}
	ttbr0_el1
}

/// Loads the current `TTBR1_EL1` register value.
///
/// # Safety
/// Caller must ensure that any references derived
/// from this function do not violate Rust's
/// aliasing (multiple-reference) rules, namely
/// when creating mutable references.
///
/// Further, depending on the execution context,
/// the page tables referred to by this register
/// may be entirely undefined.
///
/// This value is NOT guaranteed to be unique across
/// all cores in all execution contexts. Be VERY CAREFUL
/// that you control the value in the `TTBR1_EL1` register
/// prior to calling this again.
#[allow(dead_code)]
pub unsafe fn load_ttbr1() -> u64 {
	let ttbr1_el1: u64;
	unsafe {
		asm!(
			"mrs {0:x}, TTBR1_EL1",
			out(reg) ttbr1_el1
		);
	}
	ttbr1_el1
}

/// Stores a new physical address into the `TTBR0_EL1` register.
///
/// # Safety
/// Caller must ensure that the physical address is valid
/// memory, and that page tables written to that
/// address are valid.
#[allow(dead_code)]
pub unsafe fn store_ttbr0(phys: u64) {
	unsafe {
		asm!(
			"msr TTBR0_EL1, {0:x}",
			in(reg) phys,
		);
	}
}

/// Stores a new physical address into the `TTBR1_EL1` register.
///
/// # Safety
/// Caller must ensure that the physical address is valid
/// memory, and that page tables written to that
/// address are valid.
#[allow(dead_code)]
pub unsafe fn store_ttbr1(phys: u64) {
	unsafe {
		asm!(
			"msr TTBR1_EL1, {0:x}",
			in(reg) phys,
		);
	}
}
