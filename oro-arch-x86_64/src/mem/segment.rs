//! Implements address space segments, namely the mapping logic whereby the kernel
//! requests that a physical address be mapped into a specific range of virtual
//! addresses.

use super::{address_space::AddressSpaceHandle, paging::PageTable};
use crate::mem::{paging::PageTableEntry, paging_level::PagingLevel};
use oro_common::{
	mem::{
		mapper::{AddressSegment as Segment, MapError, UnmapError},
		pfa::alloc::{PageFrameAllocate, PageFrameFree},
		translate::PhysicalAddressTranslator,
	},
	unlikely,
};

/// Sign-extends a value to the appropriate size for the current paging level.
macro_rules! sign_extend {
	(L4, $value:expr) => {
		((($value << 16) as isize) >> 16) as usize
	};
	(L5, $value:expr) => {
		((($value << 7) as isize) >> 7) as usize
	};
}

pub(crate) use sign_extend;

/// A utility trait for extracting information about a mapper handle.
pub trait MapperHandle {
	/// Returns the base physical address of the page table.
	fn base_phys(&self) -> u64;
	/// Returns the paging level of the page table. This is typically
	/// cached to avoid repeated register lookups.
	fn paging_level(&self) -> PagingLevel;
}

/// A segment of the address space. This is constructed as a
/// constant value in the [`super::address_space::AddressSpaceLayout`] struct and returned
/// as a static reference.
pub struct AddressSegment {
	/// The valid range of L4/L5 indices.
	pub valid_range: (usize, usize),
	/// A template for the page table entry to use for this segment.
	/// This holds all flags except the address field, which are then
	/// copied into the actual page table entry when new entries are
	/// created.
	pub entry_template: PageTableEntry,
	/// A template for intermediate page table entries. This is used
	/// to create intermediate page tables when mapping a new address
	/// whereby level 4/3/2 entries are created.
	///
	/// For any mappings that may overlap with other segments,
	/// the entry template MUST be identical across all such segments.
	pub intermediate_entry_template: PageTableEntry,
}

impl AddressSegment {
	/// Returns the page table entry for the given virtual address,
	/// allocating intermediate page tables as necessary.
	unsafe fn entry<'a, A, P, Handle: MapperHandle>(
		&'a self,
		space: &'a Handle,
		alloc: &'a mut A,
		translator: &'a P,
		virt: usize,
	) -> Result<&'a mut PageTableEntry, MapError>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(MapError::VirtNotAligned);
		}

		{
			let root_index = match space.paging_level() {
				PagingLevel::Level4 => (virt >> 39) & 0x1FF,
				PagingLevel::Level5 => (virt >> 48) & 0x1FF,
			};
			if unlikely!(root_index < self.valid_range.0 || root_index > self.valid_range.1) {
				return Err(MapError::VirtOutOfRange);
			}
		}

		let mut current_page_table = translator.to_virtual_addr(space.base_phys());

		for level in (1..space.paging_level().as_usize()).rev() {
			let index = (virt >> (12 + level * 9)) & 0x1FF;
			let entry = &mut (&mut *(current_page_table as *mut PageTable))[index];

			current_page_table = if entry.present() {
				translator.to_virtual_addr(entry.address())
			} else {
				let frame_phys_addr = alloc.allocate().ok_or(MapError::OutOfMemory)?;

				// We zero it before placing it into the page table
				// so as to not thrash the TLB.
				let frame_virt_addr = translator.to_virtual_addr(frame_phys_addr);
				core::slice::from_raw_parts_mut(frame_virt_addr as *mut u8, 4096).fill(0);

				// SAFETY(qix-): For all intermediates, we use a common-denominator
				// SAFETY(qix-): page table entry template, which is guaranteed to
				// SAFETY(qix-): traverse for all leaf entries (executable, RO, writable,
				// SAFETY(qix-): etc). If multiple types of pages are mapped to the same
				// SAFETY(qix-): L4/3/2 segments, the first to map would otherwise dictate
				// SAFETY(qix-): the permissions for all subsequent mappings, which causes
				// SAFETY(qix-): problems.
				*entry = self
					.intermediate_entry_template
					.with_address(frame_phys_addr);

				crate::asm::invlpg(frame_virt_addr);

				frame_virt_addr
			};
		}

		let entry = &mut (&mut *(current_page_table as *mut PageTable))[(virt >> 12) & 0x1FF];

		Ok(entry)
	}

	/// Attempts to unmap a virtual address from the segment, returning the
	/// physical address that was previously mapped. Assumes that the CPU
	/// is in a 4-level paging mode.
	///
	/// If no physical address was previously mapped, returns `None`.
	// TODO(qix-): consolodate the l4 and l4 unmap functions.
	unsafe fn try_unmap_l4<A, P, Handle: MapperHandle>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
	) -> Result<Option<u64>, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(UnmapError::VirtNotAligned);
		}

		let l4_index = (virt >> 39) & 0x1FF;

		{
			if unlikely!(l4_index < self.valid_range.0 || l4_index > self.valid_range.1) {
				return Err(UnmapError::VirtOutOfRange);
			}
		}

		let l4_phys = space.base_phys();
		let l4_virt = translator.to_virtual_addr(l4_phys);
		let l4 = &mut *(l4_virt as *mut PageTable);
		let l4_entry = &mut l4[l4_index];

		Ok(if l4_entry.present() {
			let l3_phys = l4_entry.address();
			let l3_virt = translator.to_virtual_addr(l3_phys);
			let l3 = &mut *(l3_virt as *mut PageTable);
			let l3_index = (virt >> 30) & 0x1FF;
			let l3_entry = &mut l3[l3_index];

			let r = if l3_entry.present() {
				let l2_phys = l3_entry.address();
				let l2_virt = translator.to_virtual_addr(l2_phys);
				let l2 = &mut *(l2_virt as *mut PageTable);
				let l2_index = (virt >> 21) & 0x1FF;
				let l2_entry = &mut l2[l2_index];

				let r = if l2_entry.present() {
					let l1_phys = l2_entry.address();
					let l1_virt = translator.to_virtual_addr(l1_phys);
					let l1 = &mut *(l1_virt as *mut PageTable);
					let l1_index = (virt >> 12) & 0x1FF;
					let l1_entry = &mut l1[l1_index];

					let r = if l1_entry.present() {
						// NOTE: We DO NOT free the physical frame here.
						// NOTE: We let the caller do that. This is an UNMAP,
						// NOTE: not a FREE.
						let phys = l1_entry.address();
						l1_entry.reset();
						crate::asm::invlpg(virt);
						Some(phys)
					} else {
						None
					};

					if l1.empty() {
						alloc.free(l1_phys);
						l2_entry.reset();
					}

					r
				} else {
					None
				};

				if l2.empty() {
					alloc.free(l2_phys);
					l3_entry.reset();
				}

				r
			} else {
				None
			};

			if l3.empty() {
				alloc.free(l3_phys);
				l4_entry.reset();
			}

			r
		} else {
			None
		})
	}

	/// Attempts to unmap a virtual address from the segment, returning the
	/// physical address that was previously mapped. Assumes that the CPU
	/// is in a 5-level paging mode.
	///
	/// If no physical address was previously mapped, returns `None`.
	// TODO(qix-): consolodate the l4 and l4 unmap functions.
	unsafe fn try_unmap_l5<A, P, Handle: MapperHandle>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
	) -> Result<Option<u64>, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(UnmapError::VirtNotAligned);
		}

		let l5_index = (virt >> 48) & 0x1FF;

		{
			if unlikely!(l5_index < self.valid_range.0 || l5_index > self.valid_range.1) {
				return Err(UnmapError::VirtOutOfRange);
			}
		}

		let l5_phys = space.base_phys();
		let l5_virt = translator.to_virtual_addr(l5_phys);
		let l5 = &mut *(l5_virt as *mut PageTable);
		let l5_entry = &mut l5[l5_index];

		Ok(if l5_entry.present() {
			let l4_phys = l5_entry.address();
			let l4_virt = translator.to_virtual_addr(l4_phys);
			let l4 = &mut *(l4_virt as *mut PageTable);
			let l4_index = (virt >> 39) & 0x1FF;
			let l4_entry = &mut l4[l4_index];

			let r = if l4_entry.present() {
				let l3_phys = l4_entry.address();
				let l3_virt = translator.to_virtual_addr(l3_phys);
				let l3 = &mut *(l3_virt as *mut PageTable);
				let l3_index = (virt >> 30) & 0x1FF;
				let l3_entry = &mut l3[l3_index];

				let r = if l3_entry.present() {
					let l2_phys = l3_entry.address();
					let l2_virt = translator.to_virtual_addr(l2_phys);
					let l2 = &mut *(l2_virt as *mut PageTable);
					let l2_index = (virt >> 21) & 0x1FF;
					let l2_entry = &mut l2[l2_index];

					let r = if l2_entry.present() {
						let l1_phys = l2_entry.address();
						let l1_virt = translator.to_virtual_addr(l1_phys);
						let l1 = &mut *(l1_virt as *mut PageTable);
						let l1_index = (virt >> 12) & 0x1FF;
						let l1_entry = &mut l1[l1_index];

						let r = if l1_entry.present() {
							// NOTE: We DO NOT free the physical frame here.
							// NOTE: We let the caller do that. This is an UNMAP,
							// NOTE: not a FREE.
							let phys = l1_entry.address();
							l1_entry.reset();
							crate::asm::invlpg(virt);
							Some(phys)
						} else {
							None
						};

						if l1.empty() {
							alloc.free(l1_phys);
							l2_entry.reset();
						}

						r
					} else {
						None
					};

					if l2.empty() {
						alloc.free(l2_phys);
						l3_entry.reset();
					}

					r
				} else {
					None
				};

				if l3.empty() {
					alloc.free(l3_phys);
					l4_entry.reset();
				}

				r
			} else {
				None
			};

			if l4.empty() {
				alloc.free(l4_phys);
				l5_entry.reset();
			}

			r
		} else {
			None
		})
	}

	/// Maps the L4/L5 entry for the given segment range to
	/// empty page tables, without mapping any children.
	///
	/// Intended to be used to create shared segments that are
	/// otherwise empty, for later use.
	///
	/// # Safety
	/// Must only be called once per segment range.
	///
	/// Does NOT invalidate the TLB.
	pub unsafe fn make_top_level_present<A, P, Handle: MapperHandle>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
	) -> Result<(), MapError>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		let top_level = &mut *(translator.to_virtual_addr(space.base_phys()) as *mut PageTable);

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];

			if entry.present() {
				return Err(MapError::Exists);
			}

			let frame_phys_addr = alloc.allocate().ok_or(MapError::OutOfMemory)?;
			*entry = self.entry_template.with_address(frame_phys_addr);
			let frame_virt_addr = translator.to_virtual_addr(frame_phys_addr);

			(*(frame_virt_addr as *mut PageTable)).reset();
		}

		Ok(())
	}

	/// Unmaps the entire range's top level page tables without
	/// reclaiming any of the physical memory.
	///
	/// # Safety
	/// Caller must ensure that pages not being claimed _won't_
	/// lead to memory leaks.
	pub unsafe fn unmap_without_reclaim<P: PhysicalAddressTranslator, Handle: MapperHandle>(
		&self,
		space: &Handle,
		pat: &P,
	) {
		let top_level = &mut *(pat.to_virtual_addr(space.base_phys()) as *mut PageTable);

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];
			if entry.present() {
				entry.reset();
			}
		}
	}
}

unsafe impl Segment<AddressSpaceHandle> for &'static AddressSegment {
	// SAFETY(qix-): We know and understand that the sign is being munged here;
	// SAFETY(qix-): that's expected. We can safely ignore any clippy lints related to that.
	// TODO(qix-): Once const trait methods are stabilitized, make this const.
	// TODO(qix-): Once attributes on expressions are stabilized, move this directly into the macro.
	#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
	fn range(&self) -> (usize, usize) {
		// Get the current paging level.
		match PagingLevel::current_from_cpu() {
			PagingLevel::Level4 => {
				(
					sign_extend!(L4, self.valid_range.0 << 39),
					sign_extend!(L4, (self.valid_range.1 << 39) | 0x0000_007F_FFFF_FFFF),
				)
			}
			PagingLevel::Level5 => {
				(
					sign_extend!(L5, self.valid_range.0 << 48),
					sign_extend!(L5, (self.valid_range.1 << 48) | 0x0000_FFFF_FFFF_FFFF),
				)
			}
		}
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
		P: PhysicalAddressTranslator,
	{
		// NOTE(qix-): The current implementation of `entry()` doesn't
		// NOTE(qix-): actually free anyway, so we just proxy to that method.
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
		P: PhysicalAddressTranslator,
	{
		let entry = unsafe { self.entry(space, alloc, translator, virt)? };
		if entry.present() {
			return Err(MapError::Exists);
		}

		*entry = self.entry_template.with_address(phys);
		crate::asm::invlpg(virt);

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
		P: PhysicalAddressTranslator,
	{
		let phys = unsafe {
			match space.paging_level() {
				PagingLevel::Level4 => self.try_unmap_l4(space, alloc, translator, virt)?,
				PagingLevel::Level5 => self.try_unmap_l5(space, alloc, translator, virt)?,
			}
		};
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
		P: PhysicalAddressTranslator,
	{
		let entry = unsafe { self.entry(space, alloc, translator, virt)? };
		let old_phys = if entry.present() {
			Some(entry.address())
		} else {
			None
		};

		*entry = self.entry_template.with_address(phys);
		crate::asm::invlpg(virt);

		Ok(old_phys)
	}
}
