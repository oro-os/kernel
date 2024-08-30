//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment on AArch64.

use core::{arch::asm, ptr::from_ref};
use oro_arch_aarch64::{
	mair::MairEntry,
	mem::address_space::{AddressSpaceHandle, AddressSpaceLayout},
	reg::{
		self,
		tcr_el1::{
			AsidSelect, AsidSize, Cacheability, Shareability, Tg0GranuleSize, Tg1GranuleSize,
		},
	},
};
use oro_common::mem::{
	mapper::{AddressSegment, MapError},
	pfa::alloc::{PageFrameAllocate, PageFrameFree},
	translate::PhysicalAddressTranslator,
};

pub type AddressSpace = AddressSpaceLayout;
pub type SupervisorHandle = AddressSpaceHandle;

extern "C" {
	/// The start of the transfer stubs.
	static _ORO_STUBS_START: u64;
	/// The end of the transfer stubs.
	static _ORO_STUBS_LEN: u64;
}

/// The transfer token for the Aarch64 architecture.
pub struct TransferToken {
	/// The stack address for the kernel. Core-local.
	pub stack_ptr: usize,
	/// The physical address of the root page table entry for the kernel (TTBR1).
	pub ttbr1_page_table_phys: u64,
	/// The physical address of the root page table for the stubs (TTBR0)
	pub ttbr0_page_table_phys: u64,
	/// The address of the core-local stubs (identity mapped)
	pub stubs_addr: usize,
}

/// The result of mapping in the stubs
pub struct MappedStubs {
	/// The virtual address of the stubs
	pub stubs_addr: usize,
	/// The base physical address of the page table for TTBR0
	pub ttbr0_addr: u64,
}

pub struct TransferData {
	tt0_phys:   u64,
	stubs_addr: u64,
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
) -> crate::Result<TransferData> {
	debug_assert!(
		(from_ref(&_ORO_STUBS_LEN) as usize) <= 4096,
		"transfer stubs are larger than a 4KiB page"
	);

	AddressSpaceLayout::map_recursive_entry(mapper, pat);

	let stubs_phys = alloc
		.allocate()
		.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;
	let stubs_virt = pat.to_virtual_addr(stubs_phys);

	// Copy the stubs into the new page
	let stubs_dest = &mut *(stubs_virt as *mut [u8; 4096]);
	// SAFETY: We will not reference any of the data outside of the valid memory.
	#[allow(invalid_reference_casting)]
	let stubs_src = &*(from_ref(&_ORO_STUBS_START).cast::<[u8; 4096]>());
	stubs_dest.copy_from_slice(stubs_src[..(from_ref(&_ORO_STUBS_LEN) as usize)].as_ref());

	// Map the stubs into the new page table using an identity mapping.
	// SAFETY(qix-): We specify that TTBR0 must be 4KiB upon transferring to the kernel,
	// SAFETY(qix-): and that TTBR0_EL1 is left undefined (for our usage).
	let page_table = AddressSpaceLayout::new_supervisor_space_tt0(alloc, pat)
		.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;

	// Direct map it.
	#[allow(clippy::cast_possible_truncation)]
	AddressSpaceLayout::stubs()
		.map(&page_table, alloc, pat, stubs_phys as usize, stubs_phys)
		.map_err(crate::Error::MapError)?;

	Ok(TransferData {
		stubs_addr: stubs_phys,
		tt0_phys:   page_table.base_phys(),
	})
}

/// Performs the transfer from pre-boot to the kernel.
pub unsafe fn transfer(
	mapper: &mut AddressSpaceHandle,
	kernel_entry: usize,
	stack_addr: usize,
	prepare_data: TransferData,
) -> Result<!, MapError> {
	let page_table_phys: u64 = mapper.base_phys();
	let mair_value: u64 = MairEntry::build_mair().into();
	let stubs_addr: u64 = prepare_data.stubs_addr;
	let stubs_page_table_phys: u64 = prepare_data.tt0_phys;

	// Construct the final TCR_EL1 register value
	// We load the current value and modify it instead of
	// constructing a new one since several bits are reserved
	// and we don't want to accidentally overwrite them.
	let mut tcr_el1 = reg::tcr_el1::TcrEl1::load();
	// 47-bit split
	tcr_el1.set_t0sz(16);
	tcr_el1.set_t1sz(16);
	// 4KiB granule sizes
	tcr_el1.set_tg0(Tg0GranuleSize::Kb4);
	tcr_el1.set_tg1(Tg1GranuleSize::Kb4);
	// Ignore the top byte
	tcr_el1.set_tbi0(true);
	tcr_el1.set_tbi1(true);
	// 16-bit ASIDs
	tcr_el1.set_as_size(AsidSize::Bit16);
	// Use TTBR0 for ASID selection
	tcr_el1.set_a1(AsidSelect::Ttbr0);
	// Don't use 5-level paging.
	// TODO(qix-): Temporary measure to prevent any surprise behavior until it's properly implemented.
	tcr_el1.set_ds(false);
	// Set shareability and cacheability attributes
	tcr_el1.set_orgn1(Cacheability::WriteBackWriteAllocate);
	tcr_el1.set_irgn1(Cacheability::WriteBackWriteAllocate);
	tcr_el1.set_orgn0(Cacheability::WriteBackWriteAllocate);
	tcr_el1.set_irgn0(Cacheability::WriteBackWriteAllocate);
	tcr_el1.set_sh0(Shareability::OuterShareable);
	tcr_el1.set_sh1(Shareability::OuterShareable);
	// Enable translations on both halves (false = enable)
	tcr_el1.set_epd0(false);
	tcr_el1.set_epd1(false);

	let tcr_el1_raw: u64 = tcr_el1.into();

	// Disable write implies execute
	let mut sctlr = reg::sctlr_el1::SctlrEl1::load();
	sctlr.set_wxn(false);
	sctlr.write();

	// Load TTBR0_EL1 with the new page table address and flush caches
	// We do this here as opposed to the map stubs function so that e.g.
	// memory mapped devices (such as the UART) can be used right up until
	// the transfer occurs.
	asm!(
		"dsb ish",
		"isb sy",
		"msr ttbr0_el1, x0",
		"ic iallu",
		"dsb sy",
		"isb sy",
		"tlbi vmalle1is",
		"dmb sy",
		in("x0") stubs_page_table_phys,
	);

	// Tell dbgutil we're about to switch
	#[cfg(debug_assertions)]
	oro_debug::__oro_dbgutil_kernel_will_transfer();

	// Populate registers and jump to stubs
	asm!(
		"isb",
		"br x4",
		in("x0") page_table_phys,
		in("x1") stack_addr,
		in("x2") kernel_entry,
		in("x3") mair_value,
		in("x4") stubs_addr,
		in("x5") tcr_el1_raw,
		// SAFETY(qix-): Do not use `x8` or `x9` for transferring values.
		options(noreturn)
	);
}

/// Transfer stubs for the AArch64 architecture.
///
/// This function performs the actual register modifications and jumps to the kernel entry point.
///
/// # Safety
/// This function is meant to be called by the [`transfer()`] function
/// and nowhere else.
#[naked]
#[no_mangle]
#[link_section = ".oro_xfer_stubs.entry"]
unsafe extern "C" fn transfer_stubs() -> ! {
	// SAFETY(qix-): `x9` is the only temporary register usable by the stubs.
	asm!(
		// Disable MMU
		"mrs x9, sctlr_el1",
		"bic x9, x9, #1",
		"msr sctlr_el1, x9",
		// Set up the MAIR register
		"msr mair_el1, x3",
		// Set the TCR_EL1 register to the configuration expected by the kernel
		"msr tcr_el1, x5",
		// Set up the kernel page table address in TTBR1_EL1
		"msr ttbr1_el1, x0",
		// Re-enable MMU
		"mrs x9, sctlr_el1",
		"orr x9, x9, #1",
		"msr sctlr_el1, x9",
		// Invalidate TLBs
		"tlbi vmalle1is",
		// Invalidate instruction cache
		"ic iallu",
		// Invalidate data cache
		"dc isw, xzr",
		// Ensure all cache, TLB, and branch predictor maintenance operations have completed
		"dsb nsh",
		// Ensure the instruction stream is consistent
		"isb",
		// Set up the stack pointer
		"mov sp, x1",
		// Jump to the kernel entry point
		"br x2",
		options(noreturn),
	);
}
