//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment.
//!
//! These are _tightly_ coupled to the linker script.

use crate::mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel};
use core::arch::asm;

extern "C" {
	/// The start of the transfer stubs.
	pub static _ORO_STUBS_START: u64;
	/// The end of the transfer stubs.
	pub static _ORO_STUBS_LEN: u64;
}

/// Transfer token passed from `prepare_transfer` to `transfer`.
pub struct TransferToken {
	/// The stack address for the kernel. Core-local.
	pub stack_ptr:       usize,
	/// The physical address of the root page table entry for the kernel.
	pub page_table_phys: u64,
}

/// Returns the target virtual address of the stubs based on
/// the current CPU paging level.
pub fn target_address() -> usize {
	match PagingLevel::current_from_cpu() {
		PagingLevel::Level4 => AddressSpaceLayout::STUBS_IDX << 39,
		PagingLevel::Level5 => AddressSpaceLayout::STUBS_IDX << 48,
	}
}

/// Performs the transfer from pre-boot to the kernel.
///
/// # Safety
/// Only to be called ONCE per core, and only by the [`oro_common::Arch`] implementation.
pub unsafe fn transfer(entry: usize, transfer_token: &TransferToken) -> ! {
	let page_table_phys: u64 = transfer_token.page_table_phys;
	let stack_addr: usize = transfer_token.stack_ptr;
	let stubs_addr: usize = crate::xfer::target_address();

	// Jump to stubs.
	asm!(
		"push {CR3_ADDR}",
		"push {STACK_ADDR}",
		"push {KERNEL_ENTRY}",
		"jmp {STUBS_ADDR}",
		CR3_ADDR = in(reg) page_table_phys,
		STACK_ADDR = in(reg) stack_addr,
		KERNEL_ENTRY = in(reg) entry,
		STUBS_ADDR = in(reg) stubs_addr,
		options(noreturn)
	);
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
/// This function is meant to be called by the [`transfer()`]
/// function and nowhere else.
///
/// The transfer stubs MUST be 4KiB page aligned AND be a multiple of 4KiB.
#[naked]
#[no_mangle]
#[link_section = ".oro_xfer_stubs.entry"]
unsafe extern "C" fn transfer_stubs() -> ! {
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
