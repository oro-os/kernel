//! Implements the address space segment mappers for the Aarch64 architecture.
//!
//! **NOTE:** This module assumes a 4KiB granule size with a lower/higher EL0/EL1
//! translation regime during Kernel execution. It should NOT be used directly
//! in a preboot environment on the existing memory maps, as those are undefined
//! until we switch to the kernel execution context.
//!
//! It also assumes a 48-bit virtual address space (where `T0SZ`/`T1SZ` of `TCR_EL1`
//! is set to 16).

use super::paging::{PageTableEntryType, PageTableEntryTypeMut};
use crate::mem::{
	address_space::AddressSpaceHandle,
	paging::{
		L0PageTableDescriptor, L1PageTableDescriptor, L2PageTableDescriptor,
		L3PageTableBlockDescriptor, PageTable, PageTableEntry,
	},
};
use core::panic;
use oro_macro::unlikely;
use oro_mem::{
	mapper::{AddressSegment, MapError, UnmapError},
	pfa::alloc::{PageFrameAllocate, PageFrameFree},
	translate::Translator,
};

/// Sign-extends a 48-bit virtual address.
macro_rules! sign_extend {
	($value:expr) => {
		((($value << 16) as isize) >> 16) as usize
	};
}

/// A singular address space segment within which allocations are made.
///
/// Stores information about mapping flags and valid address ranges.
pub struct Segment {
	/// The valid range of L0 indices for this segment.
	///
	/// Assumes a 4KiB granule size with 48-bit virtual addresses.
	pub valid_range:       (usize, usize),
	/// For all L0 entries that get allocated within this segment,
	/// this discriptor is used as the template.
	pub l0_template:       L0PageTableDescriptor,
	/// For all L1 table entries that get allocated within this segment,
	/// this discriptor is used as the template.
	pub l1_table_template: L1PageTableDescriptor,
	/// For all L2 table entries that get allocated within this segment,
	/// this discriptor is used as the template.
	pub l2_table_template: L2PageTableDescriptor,
	/// For all L3 entries that get allocated within this segment,
	/// this discriptor is used as the template.
	pub l3_template:       L3PageTableBlockDescriptor,
}

impl Segment {
	/// Returns the page table entry for the given virtual address, mapping in
	/// intermediate page table level entries as necessary.
	/// Always returns a valid reference to an L3 page table entry (or an error
	/// if mapping intermediate table entries failed).
	// XXX DEBUG(qix-): Set this back to private
	pub(crate) fn entry<'a, A, P>(
		&'a self,
		space: &'a AddressSpaceHandle,
		alloc: &'a mut A,
		translator: &'a P,
		virt: usize,
	) -> Result<&'a mut PageTableEntry, MapError>
	where
		A: PageFrameAllocate,
		P: Translator,
	{
		let virt = virt
			.checked_sub(space.virt_start)
			.ok_or(MapError::VirtOutOfAddressSpaceRange)?;

		let l0_idx = (virt >> 39) & 0x1FF;

		if l0_idx < self.valid_range.0 || l0_idx > self.valid_range.1 {
			return Err(MapError::VirtOutOfRange);
		}

		if virt & 0xFFF != 0 {
			return Err(MapError::VirtNotAligned);
		}

		let l1_idx = (virt >> 30) & 0x1FF;
		let l2_idx = (virt >> 21) & 0x1FF;
		let l3_idx = (virt >> 12) & 0x1FF;

		// SAFETY(qix-): We have reasonable guarantees that AddressSpaceHandle's are valid.
		let l0 = unsafe { &mut *translator.translate_mut::<PageTable>(space.base_phys) };
		let l0_entry = &mut l0[l0_idx];

		let l1: &mut PageTable = if l0_entry.valid() {
			// SAFETY(qix-): We know for a fact this is the level 0; entry_type's safety concerns have been met.
			let PageTableEntryType::L0Descriptor(l0_entry) = (unsafe { l0_entry.entry_type(0) })
			else {
				panic!("L0 entry is not a descriptor");
			};

			// SAFETY(qix-): We can guarantee this is a valid page table entry.
			unsafe { &mut *translator.translate_mut(l0_entry.address()) }
		} else {
			let l1_phys = alloc.allocate().ok_or(MapError::OutOfMemory)?;
			let l1_virt = translator.translate_mut::<PageTable>(l1_phys);

			unsafe {
				// SAFETY(qix-): We can guarantee this is a valid page table address.
				(*l1_virt).reset();
				// SAFETY(qix-): If `l0_template` is malformed, we have a bug in the address layout configuration.
				// SAFETY(qix-): This is not coming from user input.
				l0_entry.set_raw(self.l0_template.with_address(l1_phys).raw());
				// SAFETY(qix-): We can guarantee this is a valid page table entry.
				&mut *l1_virt
			}
		};

		let l1_entry = &mut l1[l1_idx];

		let l2: &mut PageTable = if l1_entry.valid() {
			// SAFETY(qix-): We known for a fact this is the level 1; entry_type's safety concerns have been met.
			let PageTableEntryType::L1Descriptor(l1_entry) = (unsafe { l1_entry.entry_type(1) })
			else {
				panic!("L1 entry is not a descriptor");
			};

			// SAFETY(qix-): We can guarantee this is a valid page table entry.
			unsafe { &mut *translator.translate_mut(l1_entry.address()) }
		} else {
			let l2_phys = alloc.allocate().ok_or(MapError::OutOfMemory)?;
			let l2_virt = translator.translate_mut::<PageTable>(l2_phys);

			unsafe {
				// SAFETY(qix-): We can guarantee this is a valid page table address.
				(*l2_virt).reset();
				// SAFETY(qix-): If `l1_table_template` is malformed, we have a bug in the address layout configuration.
				// SAFETY(qix-): This is not coming from user input.
				l1_entry.set_raw(self.l1_table_template.with_address(l2_phys).raw());
				// SAFETY(qix-): We can guarantee this is a valid page table entry.
				&mut *l2_virt
			}
		};

		// SAFETY(qix-): We can guarantee this is a valid page table entry.
		let l2_entry = &mut l2[l2_idx];

		let l3: &mut PageTable = if l2_entry.valid() {
			// SAFETY(qix-): We know for a fact this is the level 2; entry_type's safety concerns have been met.
			let PageTableEntryType::L2Descriptor(l2_entry) = (unsafe { l2_entry.entry_type(2) })
			else {
				panic!("L2 entry is not a descriptor");
			};

			// SAFETY(qix-): We can guarantee this is a valid page table entry.
			unsafe { &mut *translator.translate_mut(l2_entry.address()) }
		} else {
			let l3_phys = alloc.allocate().ok_or(MapError::OutOfMemory)?;

			let l3_virt = translator.translate_mut::<PageTable>(l3_phys);

			unsafe {
				// SAFETY(qix-): We can guarantee this is a valid page table address.
				(*l3_virt).reset();
				// SAFETY(qix-): If `l2_table_template` is malformed, we have a bug in the address layout configuration.
				// SAFETY(qix-): This is not coming from user input.
				l2_entry.set_raw(self.l2_table_template.with_address(l3_phys).raw());

				// SAFETY(qix-): We can guarantee this is a valid page table entry.
				&mut *l3_virt
			}
		};

		// SAFETY(qix-): We can guarantee this is a valid page table entry.
		let l3_entry = &mut l3[l3_idx];

		Ok(l3_entry)
	}

	/// Attempts to unmap a virtual address from the segment, returning the
	/// physical address that was previously mapped.
	///
	/// If no physical address was previously mapped, returns `None`.
	unsafe fn try_unmap<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
	) -> Result<Option<u64>, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: Translator,
	{
		let virt = virt
			.checked_sub(space.virt_start)
			.ok_or(UnmapError::VirtOutOfAddressSpaceRange)?;

		if unlikely!(virt & 0xFFF != 0) {
			return Err(UnmapError::VirtNotAligned);
		}

		let l0_index = (virt >> 39) & 0x1FF;

		{
			if unlikely!(l0_index < self.valid_range.0 || l0_index > self.valid_range.1) {
				return Err(UnmapError::VirtOutOfRange);
			}
		}

		let l0_phys = space.base_phys;
		let l0 = &mut *translator.translate_mut::<PageTable>(l0_phys);
		let l0_entry = &mut l0[l0_index];

		Ok(match l0_entry.entry_type_mut(0) {
			PageTableEntryTypeMut::Invalid(_) => return Ok(None),
			PageTableEntryTypeMut::L0Descriptor(l0_entry) => {
				let l1_phys = l0_entry.address();
				let l1 = &mut *translator.translate_mut::<PageTable>(l1_phys);
				let l1_index = (virt >> 30) & 0x1FF;
				let l1_entry = &mut l1[l1_index];

				let r = match l1_entry.entry_type_mut(1) {
					PageTableEntryTypeMut::Invalid(_) => None,
					PageTableEntryTypeMut::L1Descriptor(l1_entry) => {
						let l2_phys = l1_entry.address();
						let l2 = &mut *translator.translate_mut::<PageTable>(l2_phys);
						let l2_index = (virt >> 21) & 0x1FF;
						let l2_entry = &mut l2[l2_index];

						let r = match l2_entry.entry_type_mut(2) {
							PageTableEntryTypeMut::Invalid(_) => None,
							PageTableEntryTypeMut::L2Descriptor(l2_entry) => {
								let l3_phys = l2_entry.address();
								let l3 = &mut *translator.translate_mut::<PageTable>(l3_phys);
								let l3_index = (virt >> 12) & 0x1FF;
								let l3_entry = &mut l3[l3_index];

								let r = match l3_entry.entry_type_mut(3) {
									PageTableEntryTypeMut::Invalid(_) => None,
									PageTableEntryTypeMut::L3Block(l3_entry) => {
										let phys = l3_entry.address();
										l3_entry.clear_valid();
										Some(phys)
									}
									_ => panic!("L3 entry is not a block descriptor"),
								};

								if l3.empty() {
									alloc.free(l3_phys);
									l2_entry.clear_valid();
								}

								r
							}
							_ => panic!("L2 entry is not a descriptor"),
						};

						if l2.empty() {
							alloc.free(l2_phys);
							l1_entry.clear_valid();
						}

						r
					}
					_ => panic!("L1 entry is not a descriptor"),
				};

				if l1.empty() {
					alloc.free(l1_phys);
					l0_entry.clear_valid();
				}

				r
			}
			_ => panic!("L0 entry is not a descriptor"),
		})
	}

	/// Unmaps the entire range's top level page tables without
	/// reclaiming any of the physical memory.
	///
	/// # Safety
	/// Caller must ensure that pages not being claimed _won't_
	/// lead to memory leaks.
	pub unsafe fn unmap_without_reclaim<P: Translator>(&self, space: &AddressSpaceHandle, pat: &P) {
		let top_level = &mut *pat.translate_mut::<PageTable>(space.base_phys);

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];
			if entry.valid() {
				entry.reset();
			}
		}
	}
}

unsafe impl AddressSegment<AddressSpaceHandle> for &'static Segment {
	// SAFETY(qix-): We know that the value is a 48-bit virtual address and we're
	// SAFETY(qix-): munging the sign on purpose here.
	// TODO(qix-): Once attributes on expressions is stabilized, move this to the macro instead of here.
	#[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
	fn range(&self) -> (usize, usize) {
		let start = sign_extend!(self.valid_range.0 << 39);
		// TODO(qix-): Assumes a 48-bit virtual address space for each TT; will need
		// TODO(qix-): to adjust this when other addressing modes are supported.
		let end = sign_extend!(((self.valid_range.1) << 39) | 0x0000_007F_FFFF_FFFF);
		(start, end)
	}

	fn provision_as_shared<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
	) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: Translator,
	{
		let top_level = unsafe { &mut *translator.translate_mut::<PageTable>(space.base_phys) };

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];

			if entry.valid() {
				return Err(MapError::Exists);
			}

			let frame_phys_addr = alloc.allocate().ok_or(MapError::OutOfMemory)?;
			unsafe {
				(*translator.translate_mut::<PageTable>(frame_phys_addr)).reset();
			}
			*entry = self.l0_template.with_address(frame_phys_addr).into();
		}

		Ok(())
	}

	fn map<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: Translator,
	{
		// NOTE(qix-): The mapper doesn't actually free anything,
		// NOTE(qix-): so we can just call the nofree variant.
		self.map_nofree(space, alloc, translator, virt, phys)
	}

	fn map_nofree<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: PageFrameAllocate,
		P: Translator,
	{
		let l3_entry = self.entry(space, alloc, translator, virt)?;
		if l3_entry.valid() {
			return Err(MapError::Exists);
		}

		// SAFETY(qix-): If this is invalid then we have a bug in the address layout
		// SAFETY(qix-): configuration. This is not coming from user input.
		unsafe {
			l3_entry.set_raw(self.l3_template.with_address(phys).raw());
		}

		// SAFETY(qix-): We can guarantee this is an aligned virtual address
		// SAFETY(qix-): given that `self.entry()` would have errored with a non-aligned error.
		#[expect(clippy::verbose_bit_mask)]
		unsafe {
			// Sanity check the pre-condition that it's aligned.
			debug_assert!(virt & 0xFFF == 0);
			crate::asm::invalidate_tlb_el1(virt as *const ());
		}

		Ok(())
	}

	fn unmap<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
	) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: Translator,
	{
		let phys = unsafe { self.try_unmap(space, alloc, translator, virt)? };

		phys.ok_or(UnmapError::NotMapped)
	}

	fn remap<A, P>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
		phys: u64,
	) -> Result<Option<u64>, MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: Translator,
	{
		let l3_entry = self.entry(space, alloc, translator, virt)?;

		let old_phys = if l3_entry.valid() {
			let PageTableEntryType::L3Block(l3_entry) = (unsafe { l3_entry.entry_type(3) }) else {
				panic!("L3 entry is not a block descriptor");
			};

			Some(l3_entry.address())
		} else {
			None
		};

		// SAFETY(qix-): If this is invalid then we have a bug in the address layout
		// SAFETY(qix-): configuration. This is not coming from user input.
		unsafe {
			l3_entry.set_raw(self.l3_template.with_address(phys).raw());
		}

		unsafe {
			crate::asm::invalidate_tlb_el1(virt as *const ());
		}

		Ok(old_phys)
	}
}
