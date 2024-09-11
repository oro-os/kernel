//! Contains the transfer stubs when the kernel is being switched to
//! from the preboot environment on AArch64.

use core::arch::asm;
use oro_arch_aarch64::{
	mair::MairEntry,
	mem::address_space::{AddressSpaceLayout, Ttbr1Handle},
	reg::{
		self,
		tcr_el1::{
			AsidSelect, AsidSize, Cacheability, Shareability, Tg0GranuleSize, Tg1GranuleSize,
		},
	},
};
pub use oro_arch_aarch64::{ELF_CLASS, ELF_ENDIANNESS, ELF_MACHINE};
use oro_macro::asm_buffer;
use oro_mem::{
	mapper::{AddressSegment, MapError},
	pfa::alloc::Alloc,
	translate::Translator,
};

#[expect(clippy::missing_docs_in_private_items)]
pub type AddressSpace = AddressSpaceLayout;
#[expect(clippy::missing_docs_in_private_items)]
pub type SupervisorHandle = Ttbr1Handle;

/// Passed from the [`prepare_transfer`] function to the [`transfer`] function,
/// allowing the common (arch-agnostic) boot routine to perform some finalization operations
/// between the two.
pub struct TransferData {
	/// The phyiscal address of the TTRBR0 page table
	tt0_phys:   u64,
	/// The direct-mapped physical address of the stubs
	/// to which we'll jump.
	stubs_addr: u64,
}

/// The stub machine code to be executed in order to
/// jump to the kernel.
const STUBS: &[u8] = &asm_buffer! {
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
};

/// Prepares the system for a transfer. Called before the memory map
/// is written, after which `transfer` is called.
pub unsafe fn prepare_transfer<P: Translator, A: Alloc>(
	mapper: &mut Ttbr1Handle,
	alloc: &mut A,
	pat: &P,
) -> crate::Result<TransferData> {
	debug_assert!(
		STUBS.len() <= 4096,
		"transfer stubs are larger than a 4KiB page"
	);

	debug_assert_ne!(
		STUBS.len(),
		0,
		"transfer stubs must have a length greater than 0"
	);

	AddressSpaceLayout::map_recursive_entry(mapper, pat);

	let stubs_phys = alloc
		.allocate()
		.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;

	// Copy the stubs into the new page
	let stubs_dest = &mut *pat.translate_mut::<[u8; 4096]>(stubs_phys);
	stubs_dest[..STUBS.len()].copy_from_slice(STUBS.as_ref());

	// Map the stubs into the new page table using an identity mapping.
	// SAFETY(qix-): We specify that TTBR0 must be 4KiB upon transferring to the kernel,
	// SAFETY(qix-): and that TTBR0_EL1 is left undefined (for our usage).
	let page_table = AddressSpaceLayout::new_supervisor_space_ttbr0(alloc, pat)
		.ok_or(crate::Error::MapError(MapError::OutOfMemory))?;

	// Direct map it.
	#[expect(clippy::cast_possible_truncation)]
	AddressSpaceLayout::stubs()
		.map(&page_table, alloc, pat, stubs_phys as usize, stubs_phys)
		.map_err(crate::Error::MapError)?;

	Ok(TransferData {
		stubs_addr: stubs_phys,
		tt0_phys:   page_table.base_phys,
	})
}

/// Performs the transfer from pre-boot to the kernel.
#[expect(clippy::needless_pass_by_value)]
pub unsafe fn transfer(
	mapper: &mut Ttbr1Handle,
	kernel_entry: usize,
	stack_addr: usize,
	prepare_data: TransferData,
) -> Result<!, MapError> {
	let page_table_phys: u64 = mapper.base_phys;
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
