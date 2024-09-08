//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment on x86_64.

use core::arch::asm;
use oro_arch_x86_64::mem::{
	address_space::{AddressSpaceHandle, AddressSpaceLayout},
	paging::PageTableEntry,
	paging_level::PagingLevel,
	segment::{AddressSegment, MapperHandle},
};
pub use oro_arch_x86_64::{ELF_CLASS, ELF_ENDIANNESS, ELF_MACHINE};
use oro_macro::asm_buffer;
use oro_mem::{
	mapper::{AddressSegment as _, AddressSpace as _, MapError, UnmapError},
	pfa::alloc::{PageFrameAllocate, PageFrameFree},
	translate::Translator,
};

#[expect(clippy::missing_docs_in_private_items)]
pub type SupervisorHandle = AddressSpaceHandle;
#[expect(clippy::missing_docs_in_private_items)]
pub type AddressSpace = AddressSpaceLayout;

/// The index at which the stubs are located in the address space.
/// Must be in the lower half.
const STUBS_IDX: usize = 255;

/// The x86_64 address space segment used to map stubs
/// and the root page table mapping.
const STUBS_SEGMENT_DESCRIPTOR: AddressSegment = AddressSegment {
	valid_range: (STUBS_IDX, STUBS_IDX),
	entry_template: PageTableEntry::new().with_present().with_writable(),
	intermediate_entry_template: PageTableEntry::new().with_present().with_writable(),
};

/// The stub machine code to be executed in order to
/// jump to the kernel.
const STUBS: &[u8] = &asm_buffer! {
	// Load the new page table base address.
	"mov cr3, r9",
	// Set the stack
	"mov rsp, r10",
	// Push a return value of 0 onto the stack to prevent accidental returns
	"push 0",
	"jmp r11",
};

/// Returns the target virtual address of the stubs based on
/// the current CPU paging level.
pub fn target_address() -> usize {
	match PagingLevel::current_from_cpu() {
		PagingLevel::Level4 => STUBS_IDX << 39,
		PagingLevel::Level5 => STUBS_IDX << 48,
	}
}

/// Prepares the system for a transfer. Called before the memory map
/// is written, after which `transfer` is called.
#[expect(clippy::unnecessary_wraps)]
pub unsafe fn prepare_transfer<P: Translator, A: PageFrameAllocate + PageFrameFree>(
	mapper: &mut AddressSpaceHandle,
	alloc: &mut A,
	pat: &P,
) -> crate::Result<()> {
	debug_assert!(
		STUBS.len() <= 4096,
		"transfer stubs are larger than a 4KiB page"
	);
	debug_assert_ne!(
		STUBS.len(),
		0,
		"transfer stubs must have a length greater than 0",
	);

	// Map in the recursive entry.
	AddressSpaceLayout::map_recursive_entry(mapper, pat);

	// Allocate and map in the transfer stubs
	let stubs_base = target_address();

	let source = &STUBS[0] as *const u8;
	let dest = stubs_base as *mut u8;

	let current_mapper = AddressSpaceLayout::current_supervisor_space(pat);

	let phys = alloc
		.allocate()
		.expect("failed to allocate page for transfer stubs (out of memory)");

	// Map into the target kernel page tables
	(&STUBS_SEGMENT_DESCRIPTOR)
		.map(mapper, alloc, pat, stubs_base, phys)
		.expect("failed to map page for transfer stubs for kernel address space");

	// Attempt to unmap it from the current address space.
	// If it's not mapped, we can ignore the error.
	(&STUBS_SEGMENT_DESCRIPTOR)
		.unmap(&current_mapper, alloc, pat, stubs_base)
		.or_else(|e| {
			if e == UnmapError::NotMapped {
				Ok(0)
			} else {
				Err(e)
			}
		})
		.expect("failed to unmap page for transfer stubs from current address space");

	// Now map it into the current mapper so we can access it.
	(&STUBS_SEGMENT_DESCRIPTOR)
		.map(&current_mapper, alloc, pat, stubs_base, phys)
		.expect("failed to map page for transfer stubs in current address space");

	dest.copy_from(source, STUBS.len());

	Ok(())
}

/// Performs the transfer from pre-boot to the kernel.
pub unsafe fn transfer(
	mapper: &mut AddressSpaceHandle,
	kernel_entry: usize,
	stack_addr: usize,
	_prepare_data: (),
) -> Result<!, MapError> {
	let page_table_phys: u64 = mapper.base_phys();
	let stubs_addr: usize = target_address();

	// Tell dbgutil we're about to switch
	#[cfg(debug_assertions)]
	oro_debug::__oro_dbgutil_kernel_will_transfer();

	// Jump to stubs.
	// SAFETY(qix-): Do NOT use `ax`, `bx`, `dx`, `cx` for transfer registers.
	asm!(
		"jmp r12",
		in("r9") page_table_phys,
		in("r10") stack_addr,
		in("r11") kernel_entry,
		in("r12") stubs_addr,
		options(noreturn)
	);
}
