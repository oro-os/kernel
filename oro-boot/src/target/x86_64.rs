//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment on x86_64.

use core::{arch::asm, ptr::from_ref};
use oro_arch_x86_64::mem::{
	address_space::{AddressSpaceHandle, AddressSpaceLayout},
	paging::PageTableEntry,
	paging_level::PagingLevel,
	segment::{AddressSegment, MapperHandle},
};
use oro_common::mem::{
	mapper::{AddressSegment as _, AddressSpace as _, MapError, UnmapError},
	pfa::alloc::{PageFrameAllocate, PageFrameFree},
	translate::PhysicalAddressTranslator,
};

pub type SupervisorHandle = AddressSpaceHandle;
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

extern "C" {
	/// The start of the transfer stubs.
	pub static _ORO_STUBS_START: u64;
	/// The end of the transfer stubs.
	pub static _ORO_STUBS_LEN: u64;
}

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
pub unsafe fn prepare_transfer<
	P: PhysicalAddressTranslator,
	A: PageFrameAllocate + PageFrameFree,
>(
	mapper: &mut AddressSpaceHandle,
	alloc: &mut A,
	pat: &P,
) -> crate::Result<()> {
	debug_assert!(
		(from_ref(&_ORO_STUBS_LEN) as usize) <= 4096,
		"transfer stubs are larger than a 4KiB page"
	);

	// Map in the recursive entry.
	AddressSpaceLayout::map_recursive_entry(mapper, pat);

	// Allocate and map in the transfer stubs
	let stubs_base = target_address();

	let stub_start = from_ref(&_ORO_STUBS_START) as usize;
	let stub_len = from_ref(&_ORO_STUBS_LEN) as usize;

	debug_assert!(
		stub_start & 0xFFF == 0,
		"transfer stubs must be 4KiB aligned: {stub_start:016X}",
	);
	debug_assert!(
		stub_len & 0xFFF == 0,
		"transfer stubs length must be a multiple of 4KiB: {stub_len:X}",
	);
	debug_assert!(
		stub_len > 0,
		"transfer stubs must have a length greater than 0: {stub_len:X}",
	);

	let num_pages = (stub_len + 4095) >> 12;

	let source = stub_start as *const u8;
	let dest = stubs_base as *mut u8;

	let current_mapper = AddressSpaceLayout::current_supervisor_space(pat);

	for page_offset in 0..num_pages {
		let phys = alloc
			.allocate()
			.expect("failed to allocate page for transfer stubs (out of memory)");

		let virt = stubs_base + page_offset * 4096;

		// Map into the target kernel page tables
		(&STUBS_SEGMENT_DESCRIPTOR)
			.map(mapper, alloc, pat, virt, phys)
			.expect("failed to map page for transfer stubs for kernel address space");

		// Attempt to unmap it from the current address space.
		// If it's not mapped, we can ignore the error.
		(&STUBS_SEGMENT_DESCRIPTOR)
			.unmap(&current_mapper, alloc, pat, virt)
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
			.map(&current_mapper, alloc, pat, virt, phys)
			.expect("failed to map page for transfer stubs in current address space");
	}

	dest.copy_from(source, stub_len);

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
		// Load the new page table base address.
		"mov cr3, r9",
		// Set the stack
		"mov rsp, r10",
		// Push a return value of 0 onto the stack to prevent accidental returns
		"push 0",
		"jmp r11",
		options(noreturn),
	}
}
