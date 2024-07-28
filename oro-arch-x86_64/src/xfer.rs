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
	/// The core ID.
	pub core_id:         u64,
	/// Whether or not the core is the primary core.
	pub core_is_primary: bool,
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
	let core_id: u64 = transfer_token.core_id;
	let core_is_primary: u64 = u64::from(transfer_token.core_is_primary);

	// Jump to stubs.
	// SAFETY(qix-): Do NOT use `ax`, `dx`, `cx` for transfer registers.
	asm!(
		"jmp r12",
		in("r9") page_table_phys,
		in("r10") stack_addr,
		in("r11") entry,
		in("r12") stubs_addr,
		in("r13") core_id,
		in("r14") core_is_primary,
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
		"mov cr3, r9",
		"mov rsp, r10",
		"push 0", // Push a return value of 0 onto the stack to prevent accidental returns
		"jmp r11",
		options(noreturn),
	}
}

/// Extracts important information from the registers when the kernel
/// entry point is hit, used to popular the kernel's `CoreConfig` structs.
///
/// # Safety
/// This function is ONLY meant to be called from architecture-specific
/// entry points in the kernel. DO NOT USE THIS MACRO IN PRE-BOOT ENVIRONMENTS.
#[macro_export]
macro_rules! transfer_params {
	($core_id:path, $core_is_primary:path) => {{
		::oro_common::assert_unsafe!();
		::core::arch::asm!(
			"",
			out("r13") $core_id,
			out("r14") $core_is_primary,
			options(nostack, nomem),
		);
	}};
}
