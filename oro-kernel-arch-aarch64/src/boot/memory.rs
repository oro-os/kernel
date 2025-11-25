//! Boot time memory initialization for the AArch64 architecture.

use core::arch::asm;

use oro_boot_protocol::{MemoryMapEntry, MemoryMapEntryType, memory_map::MemoryMapKind};
use oro_debug::{dbg, dbg_warn};
use oro_kernel_macro::assert;
use oro_kernel_mem::{
	global_alloc::GlobalPfa,
	phys::{Phys, PhysAddr},
};

use crate::{
	mair::MairEntry,
	mem::{
		address_space::AddressSpaceLayout,
		paging::{
			L0PageTableDescriptor, L1PageTableDescriptor, L2PageTableBlockDescriptor,
			L2PageTableDescriptor, L3PageTableBlockDescriptor, PageTable, PageTableEntry,
			PageTableEntryBlockAccessPerm, PageTableEntryTableAccessPerm,
		},
	},
};

/// Prepares the kernel memory after transfer from the boot stage
/// on AArch64.
///
/// # Safety
/// This function is inherently unsafe. It must only be called
/// once during boot.
pub unsafe fn prepare_memory() {
	// First, let's make sure the recurisive page table is set up correctly.
	const RIDX: usize = AddressSpaceLayout::RECURSIVE_ENTRY_IDX.0;
	// SAFETY(qix-): We load TTBR1 which is always safe, and then do a
	// SAFETY(qix-): safe instruction to ask the CPU to resolve the virtual address
	// SAFETY(qix-): for us, which won't fault if it fails but rather hand us
	// SAFETY(qix-): back an error code.
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

	assert!(
		at_resolution & 1 == 0,
		"recursive page table failed to resolve"
	);

	let pa = at_resolution & 0xF_FFFF_FFFF_F000;
	assert!(
		pa == ttbr1,
		"recursive page table resolved to incorrect address: {pa:016x} != {ttbr1:016x}"
	);

	let otf = OnTheFlyMapper::new();
	let mmap_iter = MemoryMapIterator::new(&otf);
	let mut pfa_iter = MemoryMapPfa::new(mmap_iter.clone());

	let linear_offset = linear_map_regions(&otf, &mut pfa_iter, mmap_iter)
		.expect("ran out of memory while linear mapping regions");

	oro_kernel_mem::translate::set_global_map_offset(linear_offset);

	// Consume the MMAP PFA and free all memory that isn't used by the
	// linear map intermediate page table entries.
	let (pfa_last_region, pfa_iter) = pfa_iter.into_inner();
	let pfa_iter = [pfa_last_region].into_iter().chain(pfa_iter);

	for region in pfa_iter {
		if region.ty == MemoryMapEntryType::Usable {
			GlobalPfa::expose_phys_range(region.base, region.length);
		}
	}

	// Now unmap the recursive entry.
	let page_table =
		Phys::from_address_unchecked(crate::asm::load_ttbr1()).as_mut_unchecked::<PageTable>();
	(*page_table)[RIDX].reset();
	(*page_table)[RIDX + 1].reset();
	(*page_table)[RIDX + 2].reset();
	(*page_table)[RIDX + 3].reset();

	// Flush everything and finish.
	crate::asm::invalid_tlb_el1_all();
}

/// Maps all regions to a linear map in the current virtual address space.
///
/// Returns the computed base offset of the page frame allocator.
///
/// Returns None if the system ran out of memory while mapping the regions.
unsafe fn linear_map_regions<'a>(
	otf: &'a OnTheFlyMapper,
	mmap_pfa: &mut MemoryMapPfa<'a>,
	regions: MemoryMapIterator<'a>,
) -> Option<u64> {
	// Get the virtual address of the linear map base.
	const LINEAR_MAP_IDX: (usize, usize) = AddressSpaceLayout::LINEAR_MAP_IDX;
	let linear_map_base = 0xFFFF_0000_0000_0000 | (LINEAR_MAP_IDX.0 << 39) as u64;
	let linear_map_last_incl = !(511 << 39) | (LINEAR_MAP_IDX.1 << 39) as u64;

	// First, we calculate the offset.
	// Adding this to the lowest physical address will give us the
	// first byte of the linear map segment.
	let mut base_offset = u64::MAX;
	for region in regions.clone() {
		base_offset = base_offset.min(region.base);
	}
	let mmap_offset = linear_map_base - base_offset;

	// We then round up to the nearest 2MiB boundary.
	let mmap_offset = (mmap_offset + ((1 << 21) - 1)) & !((1 << 21) - 1);
	debug_assert_eq!(
		mmap_offset % (1 << 21),
		0,
		"mmap_offset is not 2MiB page-aligned"
	);

	for region in regions {
		// Calculate the virtual base address (where this region
		// starts in our virtual memory).
		let mut base_phys = region.base;
		let mut length = region.length;

		// Align it to a 2MiB boundary
		let aligned = base_phys & !((1 << 21) - 1);
		let alignment_offset = base_phys - aligned;
		base_phys = aligned;
		length += alignment_offset;
		length = (length + ((1 << 21) - 1)) & !((1 << 21) - 1);

		let base_virt = base_phys + mmap_offset;

		debug_assert_eq!(
			base_virt % (1 << 21),
			0,
			"base_virt is not 2MiB page-aligned"
		);

		if base_virt < linear_map_base {
			dbg_warn!(
				"region {:016X}:{} -> {:?} is below the linear map base, skipping",
				region.base,
				region.length,
				region.ty
			);
			continue;
		}

		if (base_virt + length) > linear_map_last_incl {
			dbg_warn!(
				"region {:016X}:{} -> {:?} is above the linear map end, skipping",
				region.base,
				region.length,
				region.ty
			);
			continue;
		}

		const RIDX: usize = AddressSpaceLayout::RECURSIVE_ENTRY_IDX.0;

		let mut total_mappings = 0;

		for offset in (0..length).step_by(1 << 21) {
			let base_virt = base_virt + offset;
			let base_phys = base_phys + offset;

			// Decode the indices for the mapping.
			let l0_idx = ((base_virt >> 39) & 0x1FF) as usize;
			let l1_idx = ((base_virt >> 30) & 0x1FF) as usize;
			let l2_idx = ((base_virt >> 21) & 0x1FF) as usize;

			// Map in the L0 page table.
			// We first map it as an L3 block descriptor.
			let l0_page_table_entry_virt = 0xFFFF_0000_0000_0000
				| (RIDX << 39)
				| ((RIDX + 1) << 30)
				| ((RIDX + 2) << 21)
				| ((RIDX + 3) << 12)
				| (l0_idx * size_of::<PageTableEntry>());

			crate::asm::invalidate_tlb_el1(l0_page_table_entry_virt as *const ());

			let l0_pte = l0_page_table_entry_virt as *mut PageTableEntry;

			let l0_phys = if l0_pte.read_volatile().valid() {
				// It's current in an L0 state.
				l0_pte
					.read_volatile()
					.address(0)
					.expect("L0 PTE is not valid")
			} else {
				let phys = mmap_pfa.next()?;
				otf.zero_page(phys);
				total_mappings += 1;
				phys
			};

			// Now we write an L3 block descriptor to it so that we can address it
			// via the recursive mapping.
			l0_pte.write(
				L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_kernel_no_exec()
					.with_user_no_exec()
					.with_not_secure()
					.with_address(l0_phys)
					.into(),
			);

			let l1_page_table_entry_virt = 0xFFFF_0000_0000_0000
				| (RIDX << 39)
				| ((RIDX + 1) << 30)
				| ((RIDX + 2) << 21)
				| (l0_idx << 12)
				| (l1_idx * size_of::<PageTableEntry>());

			crate::asm::invalidate_tlb_el1(l1_page_table_entry_virt as *const ());

			let l1_pte = l1_page_table_entry_virt as *mut PageTableEntry;

			let l1_phys = if l1_pte.read_volatile().valid() {
				// It's current in an L1 state.
				l1_pte
					.read_volatile()
					.address(0)
					.expect("L1 PTE is not valid")
			} else {
				let phys = mmap_pfa.next()?;
				otf.zero_page(phys);
				total_mappings += 1;
				phys
			};

			l1_pte.write(
				L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_kernel_no_exec()
					.with_user_no_exec()
					.with_not_secure()
					.with_address(l1_phys)
					.into(),
			);

			// Now we write an L2 block descriptor back at the l0_pte location
			// so we can access the L1 page table directly.
			l0_pte.write(
				L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_kernel_no_exec()
					.with_address(l0_phys)
					.into(),
			);

			let l2_page_table_entry_virt = 0xFFFF_0000_0000_0000
				| (RIDX << 39)
				| ((RIDX + 1) << 30)
				| (l0_idx << 21)
				| (l1_idx << 12)
				| (l2_idx * size_of::<PageTableEntry>());

			crate::asm::invalidate_tlb_el1(l1_page_table_entry_virt as *const ());
			crate::asm::invalidate_tlb_el1(l2_page_table_entry_virt as *const ());

			let l2_pte = l2_page_table_entry_virt as *mut PageTableEntry;

			if !l2_pte.read_volatile().valid() {
				// Map in the linear map.
				l2_pte.write(
					L2PageTableBlockDescriptor::new()
						.with_valid()
						.with_block_access_permissions(
							PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
						)
						.with_kernel_no_exec()
						.with_mair_index(MairEntry::DirectMap.index().into())
						.with_not_secure()
						.with_user_no_exec()
						.with_address(base_phys)
						.into(),
				);
			}

			// Now to back out, we have to restore the L1 entry first...
			l0_pte.write(
				L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_kernel_no_exec()
					.with_user_no_exec()
					.with_not_secure()
					.with_address(l0_phys)
					.into(),
			);

			l1_pte.write(
				L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_kernel_no_exec()
					.with_user_no_exec()
					.with_address(l1_phys)
					.into(),
			);

			// ... and then restore the L0 entry to a real L0 entry.
			l0_pte.write(
				L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_kernel_no_exec()
					.with_user_no_exec()
					.with_address(l0_phys)
					.into(),
			);

			// NOTE(qix-): We don't need to invalidate here because given the two
			// NOTE(qix-): cases where we exit...
			// NOTE(qix-):
			// NOTE(qix-): - Either the loop finishes, and we invalidate the entire TLB.
			// NOTE(qix-): - Or the loop continues, and we invalidate the virtual addresses
			// NOTE(qix-):   before we access them.
		}

		dbg!(
			"mapped region: {:016X}:{} ({total_mappings} mappings) -> {:?} @ {base_virt:016X}",
			region.base,
			region.length,
			region.ty
		);
	}

	crate::asm::invalid_tlb_el1_all();

	Some(mmap_offset)
}

/// A rudimentary page frame allocator over a [`MemoryMapIterator`]
/// respecting the `used` field of the memory map entries.
struct MemoryMapPfa<'a> {
	/// The iterator over all memory map items.
	iterator:      MemoryMapIterator<'a>,
	/// The current entry from which we're allocating.
	current_entry: MemoryMapEntry,
}

impl<'a> MemoryMapPfa<'a> {
	/// Creates a new memory map page frame allocator.
	fn new(iterator: MemoryMapIterator<'a>) -> Self {
		Self {
			iterator,
			current_entry: MemoryMapEntry::default(),
		}
	}

	/// Consumes this iterator and returns the remaining
	/// entry and usable memory regions iterator.
	#[must_use]
	fn into_inner(self) -> (MemoryMapEntry, MemoryMapIterator<'a>) {
		(self.current_entry, self.iterator)
	}
}

impl Iterator for MemoryMapPfa<'_> {
	type Item = u64;

	fn next(&mut self) -> Option<Self::Item> {
		while self.current_entry.length < 4096
			|| self.current_entry.ty != MemoryMapEntryType::Usable
		{
			self.current_entry = self.iterator.next()?;

			// Are we page aligned?
			if !self.current_entry.base.is_multiple_of(4096) {
				let next_page = (self.current_entry.base + 4095) & !4095;
				let align = next_page - self.current_entry.base;
				self.current_entry.base += align;
				self.current_entry.length = self.current_entry.length.saturating_sub(align);
			}
		}

		debug_assert!(self.current_entry.length >= 4096);
		debug_assert!(self.current_entry.base.is_multiple_of(4096));
		debug_assert!(self.current_entry.ty == MemoryMapEntryType::Usable);

		let result = self.current_entry.base;
		self.current_entry.base += 4096;
		self.current_entry.length -= 4096;

		oro_dbgutil::__oro_dbgutil_pfa_alloc(result);

		Some(result)
	}
}

/// An iterator over the memory map that was given to us
/// by the bootloader.
///
/// Note that, depending on how the bootloader has implemented
/// populating the memory map, entries may be in any order.
/// It is thus not guaranteed that the first entries will have
/// non-zero `used` fields, followed by entries with zero
/// `used` fields, etc.
#[derive(Clone)]
struct MemoryMapIterator<'a> {
	/// The next physical address of the memory map entry.
	next: u64,
	/// The on-the-fly mapper that will service reading the entries.
	otf:  &'a OnTheFlyMapper,
}

impl<'a> MemoryMapIterator<'a> {
	/// Creates a new memory map iterator.
	fn new(otf: &'a OnTheFlyMapper) -> Self {
		Self {
			next: {
				let MemoryMapKind::V0(res) = super::protocol::MMAP_REQUEST
					.response()
					.expect("bootloader didn't provide a memory map response")
				else {
					panic!(
						"bootloader provided a memory map response, but it was of a different \
						 revision"
					);
				};

				// SAFETY(qix-): We're assuming the bootloader provided a valid memory map.
				// SAFETY(qix-): We've also used the appropriate methods from the bootloader protocol
				// SAFETY(qix-): to ensure we've gotten at least the correct revision of the memory map,
				// SAFETY(qix-): so to the best of our ability to determine the memory map is valid (though
				// SAFETY(qix-): it's really up to the bootloader to make sure it is).
				unsafe { res.assume_init_ref().next }
			},
			otf,
		}
	}
}

impl Iterator for MemoryMapIterator<'_> {
	type Item = MemoryMapEntry;

	fn next(&mut self) -> Option<Self::Item> {
		if self.next == 0 {
			return None;
		}

		// SAFETY(qix-): We're assuming the bootloader provided a valid memory map.
		// SAFETY(qix-): We've also used the appropriate methods from the bootloader protocol
		// SAFETY(qix-): to ensure we've gotten at least the correct revision of the memory map,
		// SAFETY(qix-): so to the best of our ability to determine the memory map is valid (though
		// SAFETY(qix-): it's really up to the bootloader to make sure it is).
		unsafe {
			let entry = self.otf.read_phys::<MemoryMapEntry>(self.next);
			self.next = entry.next;
			Some(entry)
		}
	}
}

/// Reads and writes physical memory by mapping in and out
/// pages on the fly via the recursive mapper.
///
/// The way this works is by writing the page table address
/// we're interacting with to `RRRR[PTE.SZ * OTF_IDX]`, where
/// `R` is the recursive index, `RRRR` is the virtual address
/// formed by putting the recursive index at all 4 (or 5) levels,
/// and `PTE.SZ * OTF_IDX` is the offset into the page table
/// of CR3 that holds the OTF's L4/L5 page table entry.
///
/// Instead of mapping fresh pages to intermediates and addressing
/// the OTF region directly, we can simply substitute the last
/// `R` in the recursive address with the OTF index, and then
/// read and write to offsets within the page table to perform
/// rudimentary memory operations on any physical address.
///
/// This is normally a very, very slow operation, but given that
/// we're only doing this for small data structures (less than a page)
/// and only to set up a linear map with as large of pages as possible,
/// the benefits outweight the cost since this offloads much of the
/// mapping requirements of the bootloaders and grants complete
/// control over the process by the kernel.
struct OnTheFlyMapper {
	/// The base address where pages will be mapped.
	base_virt:           *mut u8,
	/// The L3 page table block descriptor entry for the page.
	l3_page_table_entry: *mut L3PageTableBlockDescriptor,
}

impl OnTheFlyMapper {
	/// Creates a new OTF mapper.
	unsafe fn new() -> Self {
		// Assuming the recursive map exists (it does if we're here),
		// we can calculate the virtual address of the L1 page table
		// for the OTF region.
		const RIDX: usize = AddressSpaceLayout::RECURSIVE_ENTRY_IDX.0;
		const OTF_IDX: usize = AddressSpaceLayout::BOOT_RESERVED_IDX;

		debug_assert!(OTF_IDX < 512, "OTF_IDX is out of bounds");
		debug_assert!(RIDX < 512, "OTF_IDX is out of bounds");

		// The recursive map is in the TTBR1 register with a 48-bit address
		// and 4KiB pages. Thus, we set the highest 2 bytes to `0xFFFF`.
		// NOTE(qix-): If we ever support other levels of paging or different
		// NOTE(qix-): granule sizes, this will have to be updated.
		let base_virt = 0xFFFF_0000_0000_0000
			| (RIDX << 39)
			| ((RIDX + 1) << 30)
			| ((RIDX + 2) << 21)
			| (OTF_IDX << 12);

		let base_virt = base_virt as *mut u8;

		let l3_page_table_entry = 0xFFFF_0000_0000_0000
			| (RIDX << 39)
			| ((RIDX + 1) << 30)
			| ((RIDX + 2) << 21)
			| ((RIDX + 3) << 12)
			| (OTF_IDX * size_of::<L3PageTableBlockDescriptor>());

		let l3_page_table_entry = l3_page_table_entry as *mut L3PageTableBlockDescriptor;

		Self {
			base_virt,
			l3_page_table_entry,
		}
	}

	/// Maps in the given physical page to the OTF region slot.
	unsafe fn map_phys(&self, phys: u64) {
		debug_assert!(
			phys.is_multiple_of(4096),
			"physical address is not page-aligned"
		);
		*self.l3_page_table_entry = L3PageTableBlockDescriptor::new()
			.with_address(phys)
			.with_block_access_permissions(PageTableEntryBlockAccessPerm::KernelRWUserNoAccess)
			.with_kernel_no_exec()
			.with_user_no_exec()
			.with_not_secure()
			.with_valid();
		crate::asm::invalidate_tlb_el1(self.base_virt.cast_const());
	}

	/// Reads a value from somewhere in physical memory.
	/// Might read from two pages in the event the value
	/// spans a page boundary.
	///
	/// # Safety
	/// The physical page must be valid, and in the event the type spaces
	/// the end of the page boundary, the following physical address must
	/// also be valid.
	unsafe fn read_phys<T: Sized + Copy>(&self, addr: u64) -> T {
		assert::fits::<T, 4096>();
		let offset = addr % 4096;
		let phys_base = addr - offset;
		let end_offset = offset + size_of::<T>() as u64;
		self.map_phys(phys_base);

		let mut result = core::mem::MaybeUninit::<T>::uninit();

		let this_page_end_offset = end_offset.min(4096);
		let first_page_size = (this_page_end_offset - offset) as usize;

		// TODO(qix-): Once `maybe_uninit_fill` and `maybe_uninit_as_bytes`
		// TODO(qix-): are stabilized, use those instead.
		// TODO(qix-): https://github.com/rust-lang/rust/issues/93092
		// TODO(qix-): https://github.com/rust-lang/rust/issues/117428
		for i in 0..first_page_size {
			// We have to do a volatile read because Rust _might_
			// think that the memory is the same as before; however
			// that's not true given that we're changing the backing
			// memory from under it.
			*result.as_mut_ptr().cast::<u8>().add(i) =
				self.base_virt.add(offset as usize + i).read_volatile();
		}

		if end_offset > 4096 {
			let next_page_end_offset = end_offset - 4096;
			// We should never span more than one page boundary given that
			// we enforce the read sizes to be max 4096 bytes.
			debug_assert!(
				next_page_end_offset <= 4096,
				"something went wrong with our math / page size constraints"
			);
			self.map_phys(phys_base + 4096);

			let result_offset = size_of::<T>() - first_page_size;
			for i in 0..next_page_end_offset as usize {
				*result.as_mut_ptr().cast::<u8>().add(result_offset + i) =
					self.base_virt.add(i).read_volatile();
			}
		}

		result.assume_init()
	}

	/// Writes zeros to a 4KiB physical page.
	///
	/// # Safety
	/// The physical page must be valid and
	/// page aligned.
	unsafe fn zero_page(&self, addr: u64) {
		debug_assert_eq!(addr % 4096, 0, "physical address is not page-aligned");

		self.map_phys(addr);
		for i in 0..4096 {
			self.base_virt.add(i).write_volatile(0);
		}
	}
}
