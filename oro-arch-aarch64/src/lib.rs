//! AArch64 abstraction layer for the Oro operating system kernel.
//!
//! All functionality in this crate is AArch64 specific but entirely
//! _platform_ agnostic. Oro-specific functionality should go
//! into `oro-kernel-arch-aarch64`.
#![no_std]
#![cfg_attr(doc, feature(doc_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(ptr_as_ref_unchecked)]
#![expect(clippy::inline_always)]

use core::arch::asm;

pub mod mem;
pub mod psci;
pub mod reg;

/// Invalidates the TLB entry for the given virtual address.
///
/// # Safety
/// Caller must ensure that the virtual address is valid and aligned.
pub unsafe fn invalidate_tlb_el1<T>(virt: *const T) {
	unsafe {
		asm!(
			"dsb ish",                // Ensure the update is visible
			"dc ivac, {0:x}",         // Invalidate the data cache by virtual address
			"ic ivau, {0:x}",         // Invalidate the instruction cache by virtual address
			"tlbi vaae1, {0}",        // Invalidate the TLB entry by virtual address for EL1
			"dsb ish",                // Ensure completion of the invalidation
			"isb",                    // Synchronize the instruction stream
			in(reg) virt as u64,
			options(nostack, preserves_flags),
		);
	}
}

/// Invalidates the entire TLB.
pub fn invalidate_tlb_el1_all() {
	unsafe {
		asm!(
			"tlbi vmalle1",
			"dsb ish",
			"isb",
			options(nostack, preserves_flags),
		);
	}
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
#[must_use]
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
#[must_use]
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
pub unsafe fn store_ttbr0(phys: u64) {
	unsafe {
		asm!(
			"msr TTBR0_EL1, {0:x}",
			in(reg) phys,
		);
	}
}

/// Disables interrupts on the current core.
pub fn disable_interrupts() {
	unsafe {
		asm!("msr daifset, 0xf", options(nostack, nomem, preserves_flags));
	}
}

/// Halts the processor forever.
pub fn halt() -> ! {
	loop {
		halt_once_and_wait();
	}
}

/// Halts the processor once, waiting for an interrupt.
pub fn halt_once_and_wait() {
	unsafe {
		asm!("wfi");
	}
}

/// Performs a data synchronization barrier.
#[inline(always)]
pub fn strong_memory_barrier() {
	unsafe {
		asm!("dsb sy", options(nostack, preserves_flags),);
	}
}
