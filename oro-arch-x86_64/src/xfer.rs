//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment.
//!
//! These are _tightly_ coupled to the linker script.

use crate::{
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
	reg::Cr0,
};
use core::arch::asm;
use oro_common::mem::mapper::AddressSegment;

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
/// Only to be called ONCE per core, and only by the
/// [`oro_common::arch::Arch`] implementation.
pub unsafe fn transfer(
	entry: usize,
	transfer_token: &TransferToken,
	boot_config_virt: usize,
	pfa_head: u64,
) -> ! {
	let page_table_phys: u64 = transfer_token.page_table_phys;
	let stack_addr: usize = transfer_token.stack_ptr;
	let stubs_addr: usize = crate::xfer::target_address();
	let core_id: u64 = transfer_token.core_id;
	let core_is_primary: u64 = u64::from(transfer_token.core_is_primary);
	let gdt_base: usize = AddressSpaceLayout::gdt().range().0;

	// Tell dbgutil we're about to switch
	#[cfg(debug_assertions)]
	crate::dbgutil::__oro_dbgutil_kernel_will_transfer();

	// Jump to stubs.
	// SAFETY(qix-): Do NOT use `ax`, `bx`, `dx`, `cx` for transfer registers.
	asm!(
		"jmp r12",
		in("r8") pfa_head,
		in("r9") page_table_phys,
		in("r10") stack_addr,
		in("r11") entry,
		in("r12") stubs_addr,
		in("r13") core_id,
		in("r14") core_is_primary,
		in("r15") boot_config_virt,
		in("rdi") gdt_base,
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
	#[allow(clippy::missing_docs_in_private_items)]
	const CR0_BITS: u64 = Cr0::new()
		.with_monitor_coprocessor()
		.with_emulation()
		.with_write_protect()
		.with_alignment_mask()
		.with_paging_enable()
		.bits();

	#[allow(clippy::missing_docs_in_private_items)]
	const CR0_MASK: u64 = Cr0::mask();

	asm! {
		// Load the new page table base address.
		"mov cr3, r9",
		// Load the GDT. Doesn't do anything until
		// segment registers are re-loaded.
		"lgdt [rdi]",
		// Set non-code segment registers
		"mov ax, 0x10",
		"mov ds, ax",
		"mov es, ax",
		"mov fs, ax",
		"mov gs, ax",
		"mov ss, ax",
		// Set the stack
		"mov rsp, r10",
		// CS is at offset 0x08, and we can't just move into CS,
		// so we must push the segment selector onto the stack and
		// then return to it.
		"sub rsp, 16",
		"mov qword ptr[rsp + 8], 0x08",
		"lea rax, [rip + 2f]",
		"mov qword ptr[rsp], rax",
		"retfq",
		// Using 2f instead of 0/1 due to LLVM bug
		// (https://bugs.llvm.org/show_bug.cgi?id=36144)
		// causing them to be parsed as binary literals
		// under intel syntax.
		"2:",
		// Load CR0 with the new bits.
		"mov r9, cr0",
		"mov rax, {CR0_MASK}",
		"and r9, rax",
		"mov rax, {CR0_BITS}",
		"or r9, rax",
		"mov cr0, r9",
		// Push a return value of 0 onto the stack to prevent accidental returns
		"push 0",
		"jmp r11",
		CR0_BITS = const CR0_BITS,
		CR0_MASK = const CR0_MASK,
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
	($core_id:path, $core_is_primary:path, $boot_config_virt:path, $pfa_head:path) => {{
		::oro_common::assert_unsafe!();
		::core::arch::asm!(
			"",
			out("r8") $pfa_head,
			out("r13") $core_id,
			out("r14") $core_is_primary,
			out("r15") $boot_config_virt,
			options(nostack, nomem),
		);
	}};
}
