//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment.
//!
//! These are _tightly_ coupled to the linker script.

use crate::mem::{layout::Layout, paging_level::PagingLevel};
use core::arch::asm;

extern "C" {
	/// The start of the transfer stubs.
	pub static _ORO_STUBS_START: u64;
	/// The end of the transfer stubs.
	pub static _ORO_STUBS_LEN: u64;
}

/// Transfer stubs for the x86_64 architecture.
///
/// The following values need to be pushed onto the stack before
/// jumping to the stubs. Push them *in this order*; do not reverse them
///
/// - The new page table base address (cr3).
/// - The new stack pointer.
/// - The new instruction pointer.
///
/// # Safety
/// This function is meant to be called by the [`crate::Arch::transfer()`]
/// and nowhere else.
///
/// The transfer stubs MUST be 4KiB page aligned AND be a multiple of 4KiB.
#[naked]
#[no_mangle]
#[link_section = ".oro_xfer_stubs.entry"]
pub unsafe extern "C" fn transfer_stubs() -> ! {
	asm! {
		"pop r10",
		"pop r9",
		"pop r8",
		"mov cr3, r8",
		"mov rsp, r9",
		"push 0", // Push a return value of 0 onto the stack to prevent accidental returns
		"jmp r10",
		options(noreturn)
	}
}

/// Returns the target virtual address of the stubs based on
/// the current CPU paging level.
pub fn target_address() -> usize {
	match PagingLevel::current_from_cpu() {
		PagingLevel::Level4 => Layout::STUBS_IDX << 39,
		PagingLevel::Level5 => Layout::STUBS_IDX << 48,
	}
}
