use crate::mem::{
	address_space::AddressSpaceLayout,
	paging::{PageTableEntry, PageTableEntryType},
};
use core::arch::asm;
use oro_common::mem::{
	pfa::filo::FiloPageFrameAllocator, translate::OffsetPhysicalAddressTranslator,
};

pub struct PreparedMemory {
	pub pat: OffsetPhysicalAddressTranslator,
	pub pfa: FiloPageFrameAllocator<OffsetPhysicalAddressTranslator>,
}

pub fn prepare_memory() -> PreparedMemory {
	// First, let's make sure the recurisive page table is set up correctly.
	const RIDX: usize = AddressSpaceLayout::RECURSIVE_ENTRY_IDX.0;
	// SAFETY(qix-): We load TTBR1 which is always safe, and then do a
	// SAFETY(qix-): safe instruction to ask the CPU to resolve the virtual address
	// SAFETY(qix-): for us, which won't fault if it fails but rather hand us
	// SAFETY(qix-): back an error code.
	unsafe {
		let ttbr1 = crate::asm::load_ttbr1();
		let addr = (0xFFFF << 48)
			| (RIDX << 39)
			| ((RIDX + 1) << 30)
			| ((RIDX + 2) << 21)
			| ((RIDX + 3) << 12);
		let at_resolution: u64;
		asm!(
			"AT S1E1R, {0}",
			"MRS {1}, PAR_EL1",
			in(reg) addr,
			out(reg) at_resolution,
			options(nostack, preserves_flags),
		);

		if at_resolution & 1 != 0 {
			panic!("recursive page table failed to resolve");
		}

		let pa = at_resolution & 0xF_FFFF_FFFF_F000;
		if pa != ttbr1 {
			panic!("recursive page table resolved to incorrect address: {pa:016x} != {ttbr1:016x}");
		}
	}

	// TODO
	return unsafe { core::mem::zeroed() };
}
