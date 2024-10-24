//! Memory initialization routines for the x86_64 architecture.
//!
//! Sets up the system's memory for the kernel to use and returns
//! several memory facilities usable by the kernel (e.g. a page frame
//! allocator, linear map translator, etc.).

use oro_boot_protocol::{MemoryMapEntry, MemoryMapEntryType, memory_map::MemoryMapKind};
use oro_debug::{dbg, dbg_warn};
use oro_macro::assert;
use oro_mem::{
	mapper::AddressSegment,
	pfa::{Alloc, FiloPageFrameAllocator},
	phys::{Phys, PhysAddr},
};

use crate::mem::{
	address_space::AddressSpaceLayout,
	paging::{PageTable, PageTableEntry},
	paging_level::PagingLevel,
};

/// The index of the page table entry at the highest (4/5) level
/// that is used for the on-the-fly mapper. It doesn't really
/// matter which index is used as long as it's in the lower
/// half of the address space (and doesn't conflict with any
/// of the official [`crate::mem::address_space::AddressSpaceLayout`] indices).
const OTF_IDX: usize = 254;

/// 1MiB of memory.
const MIB_1: u64 = 1024 * 1024;

/// Result from the [`prepare_memory`] function.
pub struct PreparedMemory {
	/// The page frame allocator.
	pub pfa:      FiloPageFrameAllocator,
	/// Whether or not physical pages 0x8000 and 0x9000 are available,
	/// which are required to boot secondary cores.
	pub has_cs89: bool,
}

/// Prepares the system's memory. Namely, it performs the following:
///
/// - Validates that the bootloader set us up a recursive mapping
///   at index 256.
/// - Validates that the bootloader handed us a non-empty memory map.
/// - Uses the recursive mapping to linear map all physical memory
///   to the supervisor space.
/// - Creates a page frame allocator with the newly validated linear map
///   offset, and uses it to free all memory that isn't 1) used by
///   the bootloader, and 2) isn't used by the linear map intermediate
///   page table entries.
/// - Unmaps all memory in the lower half of the address space, as per
///   the architecture boot routine specification for x86_64.
/// - Uninstalls the recursive mapping.
/// - Hands back a physical address translator and page frame allocator
///   for the system to use.
pub unsafe fn prepare_memory() -> PreparedMemory {
	// First, let's make sure the recursive entry is mapped.
	#[expect(clippy::missing_docs_in_private_items)]
	const RIDX: usize = crate::mem::address_space::AddressSpaceLayout::RECURSIVE_IDX;
	let cr3 = crate::asm::cr3();
	let paging_level = PagingLevel::current_from_cpu();

	let mut current_level = paging_level as usize;
	let mut current_addr = 0;

	while current_level > 0 {
		current_level -= 1;
		current_addr |= RIDX << (current_level * 9 + 12);
	}

	let recursive_virt = match paging_level {
		PagingLevel::Level4 => crate::mem::segment::sign_extend!(L4, current_addr),
		PagingLevel::Level5 => crate::mem::segment::sign_extend!(L5, current_addr),
	};

	let pt = &*(recursive_virt as *const crate::mem::paging::PageTable);

	// Realistically speaking, this panic probably won't even
	// be reached if it's not mapped, as we'd be incurring a page fault
	// anyway (and interrupts haven't been installed yet).
	assert!(
		pt[RIDX].present() && pt[RIDX].address() == cr3,
		"recursive entry not mapped"
	);

	let otf_mapper = OnTheFlyMapper::new();
	let mmap_iterator = MemoryMapIterator::new(&otf_mapper);
	let mut has_cs8 = false;
	let mut has_cs9 = false;

	for region in mmap_iterator.clone() {
		if region.base < MIB_1 {
			let end = region.base + region.length;

			if region.base <= 0x8000 && end >= 0x9000 {
				has_cs8 = true;
			}
			if region.base <= 0x9000 && end >= 0xA000 {
				has_cs9 = true;
			}
		}
	}

	// Next we use a recursive mapper specifically for the linear map.
	//
	// It works by iterating over the memory map provided to us by the
	// bootloader using two iterators - the first is obviously to map
	// the regions, but the second is used to allocate the page frames
	// needed to map the actual regions. This second iterator first
	// skips over all of the regions that are marked 'reclaimable',
	// and then maps uses next available `Usable` region to allocate
	// the intermediate page table entry frames.
	//
	// We'll then get back an iterator of the remaining usable regions
	// and free them back into a new page frame allocator, thus resulting
	// in a primed PFA with all actual usable memory regions made available.
	let mut mmap_pfa = MemoryMapPfa::new(mmap_iterator.clone());
	let linear_offset = linear_map_regions(&otf_mapper, &mut mmap_pfa, mmap_iterator)
		.expect("system ran out of memory during linear map");

	oro_mem::translate::set_global_map_offset(linear_offset);

	let mut pfa = FiloPageFrameAllocator::new();

	// Consume the MMAP PFA and free all memory that isn't used by the
	// linear map intermediate page table entries.
	let (pfa_last_region, pfa_iter) = mmap_pfa.into_inner();
	let pfa_iter = [pfa_last_region].into_iter().chain(pfa_iter);

	for region in pfa_iter {
		if region.ty != MemoryMapEntryType::Usable {
			continue;
		}

		let base = region.base;
		let aligned_base = (base + 4095) & !4095;
		let length = region.length.saturating_sub(aligned_base - base);

		debug_assert_eq!(aligned_base % 4096, 0);
		debug_assert_eq!(length % 4096, 0);

		#[cfg(debug_assertions)]
		{
			oro_dbgutil::__oro_dbgutil_pfa_will_mass_free(1);
			oro_dbgutil::__oro_dbgutil_pfa_mass_free(aligned_base, aligned_base + length);
		}

		for page in (aligned_base..(aligned_base + length)).step_by(4096) {
			pfa.free(page);
		}

		#[cfg(debug_assertions)]
		oro_dbgutil::__oro_dbgutil_pfa_finished_mass_free();
	}

	// Uninstall the recursive mapping.
	let l4 = Phys::from_address_unchecked(crate::asm::cr3()).as_mut_unchecked::<PageTable>();
	l4[RIDX].reset();

	// Unmap anything in the lower half.
	// We do not need to reclaim any of the memory with a PFA;
	// it is not allowed to be marked as used by the PFA beforehand
	// (as specified by the x86_64 booting specification, see crate
	// documentation) and thus the kernel will simply overwrite it
	// automatically.
	for l4_idx in 0..=255 {
		l4[l4_idx].reset();
	}

	// Flush the TLB
	crate::asm::flush_tlb();

	PreparedMemory {
		pfa,
		has_cs89: has_cs8 && has_cs9,
	}
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
	let paging_level = PagingLevel::current_from_cpu();

	#[expect(clippy::missing_docs_in_private_items)]
	macro_rules! extend {
		($virt:expr) => {
			match paging_level {
				PagingLevel::Level4 => crate::mem::segment::sign_extend!(L4, $virt),
				PagingLevel::Level5 => crate::mem::segment::sign_extend!(L5, $virt),
			}
		};
	}

	// Get the virtual address of the linear map base.
	let linear_map_segment = AddressSpaceLayout::linear_map();
	let (linear_map_base, linear_map_last_incl) = linear_map_segment.range();
	let (linear_map_base, linear_map_last_incl) =
		(linear_map_base as u64, linear_map_last_incl as u64);

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

	// We hack in a synthetic region to map the first
	// 4GiB of memory to the linear map, since much
	// of the ACPI tables and other MMIO are located
	// there and bootloaders tend not to report them
	// to us in the memory map.
	let regions = [MemoryMapEntry {
		base:   0,
		length: 1 << 32,
		ty:     MemoryMapEntryType::Unknown,
		next:   0,
	}]
	.into_iter()
	.chain(regions);

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

		let mut base_virt = base_phys + mmap_offset;

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

		#[expect(clippy::missing_docs_in_private_items)]
		const RIDX: usize = crate::mem::address_space::AddressSpaceLayout::RECURSIVE_IDX;

		let start_of_region = base_virt;

		let mut total_mappings = 0;
		while length > 0 {
			for level in (2..=paging_level as u64).rev() {
				let mut page_table_virt = (base_virt >> (12 + 9 * level)) as usize;
				for rec_level in 0..level {
					let shift = 9 * (rec_level + ((paging_level as u64) - level));
					page_table_virt &= !(0x1FF << shift);
					page_table_virt |= RIDX << shift;
				}

				page_table_virt = extend!(page_table_virt << 12);

				let page_table = &mut *(page_table_virt as *mut PageTable);
				let entry_idx = base_virt >> (12 + 9 * (level - 1)) & 0x1FF;
				let entry = &mut page_table[entry_idx as usize];

				if level == 2 {
					// `entry.present() == true` occurs when two regions that
					// have been 2MiB extended end up overlapping. In this
					// case, cool! We've already mapped this region, we can do nothing.
					if !entry.present() {
						// Make a 2MiB page.
						debug_assert_eq!(base_phys % (1 << 21), 0, "base_phys is not 2MiB aligned");
						*entry = PageTableEntry::new()
							.with_writable()
							.with_present()
							.with_global()
							.with_no_exec()
							.with_huge()
							.with_address(base_phys);
						total_mappings += 1;
					}
				} else if !entry.present() {
					let pt_phys = mmap_pfa.next()?;
					// Do this _before_ putting it into the PTE to prevent
					// TLB thrashing in some cases.
					otf.zero_page(pt_phys);
					*entry = PageTableEntry::new()
						.with_writable()
						.with_present()
						.with_global()
						.with_no_exec()
						.with_address(pt_phys);
					total_mappings += 1;
				}
			}

			base_virt += 1 << 21;
			base_phys += 1 << 21;
			length -= 1 << 21;
		}

		dbg!(
			"mapped region: {:016X}:{} ({total_mappings} mappings) -> {:?} @ \
			 {start_of_region:016X}",
			region.base,
			region.length,
			region.ty
		);
	}

	crate::asm::flush_tlb();

	Some(mmap_offset)
}

/// A rudimentary page frame allocator over a [`MemoryMapIterator`].
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

			// Skip over any regions that are below the 1MiB mark.
			if self.current_entry.base < MIB_1 {
				let bytes_to_1mib = MIB_1 - self.current_entry.base;
				self.current_entry.base += bytes_to_1mib;
				self.current_entry.length = self.current_entry.length.saturating_sub(bytes_to_1mib);
			}

			// Are we page aligned?
			if self.current_entry.base % 4096 != 0 {
				let next_page = (self.current_entry.base + 4095) & !4095;
				let align = next_page - self.current_entry.base;
				self.current_entry.base += align;
				self.current_entry.length = self.current_entry.length.saturating_sub(align);
			}
		}

		debug_assert!(self.current_entry.length >= 4096);
		debug_assert!((self.current_entry.base % 4096) == 0);
		debug_assert!(self.current_entry.ty == MemoryMapEntryType::Usable);

		let result = self.current_entry.base;
		self.current_entry.base += 4096;
		self.current_entry.length -= 4096;

		#[cfg(debug_assertions)]
		oro_dbgutil::__oro_dbgutil_pfa_alloc(result);

		Some(result)
	}
}

/// An iterator over the memory map that was given to us
/// by the bootloader.
///
/// Note that, depending on how the bootloader has implemented
/// populating the memory map, entries may be in any order.
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
				unsafe { core::ptr::read_volatile(&res.assume_init_ref().next) }
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
	/// The L1 (leaf) page table entry for the page.
	l1_page_table_entry: *mut PageTableEntry,
}

impl OnTheFlyMapper {
	/// Creates a new OTF mapper.
	unsafe fn new() -> Self {
		// Assuming the recursive map exists (it does if we're here),
		// we can calculate the virtual address of the L1 page table
		// for the OTF region.
		#[expect(clippy::missing_docs_in_private_items)]
		const RIDX: usize = crate::mem::address_space::AddressSpaceLayout::RECURSIVE_IDX;
		let paging_level = PagingLevel::current_from_cpu();
		let levels = paging_level as usize;

		let mut current_level = levels;
		let mut base_virt = 0;
		while current_level > 1 {
			current_level -= 1;
			base_virt |= RIDX << (current_level * 9 + 12);
		}
		base_virt |= OTF_IDX << 12;
		let base_virt = match paging_level {
			PagingLevel::Level4 => crate::mem::segment::sign_extend!(L4, base_virt),
			PagingLevel::Level5 => crate::mem::segment::sign_extend!(L5, base_virt),
		};
		let base_virt = base_virt as *mut u8;

		current_level = levels;
		let mut l1_page_table = 0;
		while current_level > 0 {
			current_level -= 1;
			l1_page_table |= RIDX << (current_level * 9 + 12);
		}
		let l1_page_table_entry = l1_page_table + OTF_IDX * core::mem::size_of::<PageTableEntry>();
		let l1_page_table_entry = match paging_level {
			PagingLevel::Level4 => crate::mem::segment::sign_extend!(L4, l1_page_table_entry),
			PagingLevel::Level5 => crate::mem::segment::sign_extend!(L5, l1_page_table_entry),
		};
		// SAFETY: We're assuming the recursive map exists, so we can
		// SAFETY: safely dereference the L1 page table entry.
		let l1_page_table_entry = l1_page_table_entry as *mut PageTableEntry;

		Self {
			base_virt,
			l1_page_table_entry,
		}
	}

	/// Maps in the given physical page to the OTF region slot.
	unsafe fn map_phys(&self, phys: u64) {
		debug_assert!(phys % 4096 == 0, "physical address is not page-aligned");
		*self.l1_page_table_entry = PageTableEntry::new()
			.with_present()
			.with_writable()
			.with_write_through()
			.with_no_exec()
			.with_address(phys);
		crate::asm::invlpg(self.base_virt);
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
		let end_offset = offset + core::mem::size_of::<T>() as u64;
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

			let result_offset = core::mem::size_of::<T>() - first_page_size;
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
