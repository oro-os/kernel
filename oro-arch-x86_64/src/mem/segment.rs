//! Implements address space segments, namely the mapping logic whereby the kernel
//! requests that a physical address be mapped into a specific range of virtual
//! addresses.

use oro_macro::unlikely;
use oro_mem::{
	mapper::{AddressSegment as Segment, MapError, UnmapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use super::{address_space::AddressSpaceHandle, paging::PageTable};
use crate::mem::{paging::PageTableEntry, paging_level::PagingLevel};

/// Sign-extends a value to the appropriate size for the current paging level.
macro_rules! sign_extend {
	(L4, $value:expr) => {
		// SAFETY(qix-): We expect and want the sign loss here.
		#[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
		{
			((($value << 16) as isize) >> 16) as usize
		}
	};
	(L5, $value:expr) => {
		// SAFETY(qix-): We expect and want the sign loss here.
		#[expect(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
		{
			((($value << 7) as isize) >> 7) as usize
		}
	};
}

pub(crate) use sign_extend;

/// A utility trait for extracting information about a mapper handle.
pub trait MapperHandle {
	/// Returns the base physical address of the page table.
	fn base_phys(&self) -> Phys;
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
	unsafe fn entry<'a, A, Handle: MapperHandle>(
		&'a self,
		space: &'a Handle,
		alloc: &'a mut A,
		virt: usize,
	) -> Result<&'a mut PageTableEntry, MapError>
	where
		A: Alloc,
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

		let mut current_page_table = space.base_phys().as_mut_ptr_unchecked::<PageTable>();

		for level in (1..space.paging_level().as_usize()).rev() {
			let index = (virt >> (12 + level * 9)) & 0x1FF;
			let entry = &mut (&mut *current_page_table)[index];

			current_page_table = if entry.present() {
				Phys::from_address_unchecked(entry.address()).as_mut_ptr_unchecked()
			} else {
				let frame_phys_addr = alloc.allocate().ok_or(MapError::OutOfMemory)?;

				// We zero it before placing it into the page table
				// so as to not thrash the TLB.
				let frame_phys = Phys::from_address_unchecked(frame_phys_addr);
				let frame_virt_addr = frame_phys.as_mut_ptr_unchecked::<u8>();
				frame_virt_addr.write_bytes(0, 4096);

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
				#[expect(clippy::cast_ptr_alignment)]
				let pt = frame_virt_addr.cast::<PageTable>();
				debug_assert!(pt.is_aligned());
				pt
			};
		}

		let entry = &mut (*current_page_table)[(virt >> 12) & 0x1FF];

		Ok(entry)
	}

	/// Attempts to unmap a virtual address from the segment, returning the
	/// physical address that was previously mapped. Assumes that the CPU
	/// is in a 4-level paging mode.
	///
	/// If no physical address was previously mapped, returns `None`.
	// TODO(qix-): consolodate the l4 and l4 unmap functions.
	unsafe fn try_unmap_l4<A, Handle: MapperHandle>(
		&self,
		space: &Handle,
		alloc: &mut A,
		virt: usize,
	) -> Result<Option<u64>, UnmapError>
	where
		A: Alloc,
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
		let l4 = l4_phys.as_mut_unchecked::<PageTable>();
		let l4_entry = &mut l4[l4_index];

		Ok(if l4_entry.present() {
			let l3_phys = l4_entry.address();
			let l3 = Phys::from_address_unchecked(l3_phys).as_mut_unchecked::<PageTable>();
			let l3_index = (virt >> 30) & 0x1FF;
			let l3_entry = &mut l3[l3_index];

			let r = if l3_entry.present() {
				let l2_phys = l3_entry.address();
				let l2 = Phys::from_address_unchecked(l2_phys).as_mut_unchecked::<PageTable>();
				let l2_index = (virt >> 21) & 0x1FF;
				let l2_entry = &mut l2[l2_index];

				let r = if l2_entry.present() {
					let l1_phys = l2_entry.address();
					let l1 = Phys::from_address_unchecked(l1_phys).as_mut_unchecked::<PageTable>();
					let l1_index = (virt >> 12) & 0x1FF;
					let l1_entry = &mut l1[l1_index];

					let r = if l1_entry.present() {
						// NOTE: We DO NOT free the physical frame here.
						// NOTE: We let the caller do that. This is an UNMAP,
						// NOTE: not a FREE.
						let phys = l1_entry.address();
						l1_entry.reset();
						crate::asm::invlpg(virt as *const ());
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
	unsafe fn try_unmap_l5<A, Handle: MapperHandle>(
		&self,
		space: &Handle,
		alloc: &mut A,
		virt: usize,
	) -> Result<Option<u64>, UnmapError>
	where
		A: Alloc,
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
		let l5 = l5_phys.as_mut_unchecked::<PageTable>();
		let l5_entry = &mut l5[l5_index];

		Ok(if l5_entry.present() {
			let l4_phys = l5_entry.address();
			let l4 = Phys::from_address_unchecked(l4_phys).as_mut_unchecked::<PageTable>();
			let l4_index = (virt >> 39) & 0x1FF;
			let l4_entry = &mut l4[l4_index];

			let r = if l4_entry.present() {
				let l3_phys = l4_entry.address();
				let l3 = Phys::from_address_unchecked(l3_phys).as_mut_unchecked::<PageTable>();
				let l3_index = (virt >> 30) & 0x1FF;
				let l3_entry = &mut l3[l3_index];

				let r = if l3_entry.present() {
					let l2_phys = l3_entry.address();
					let l2 = Phys::from_address_unchecked(l2_phys).as_mut_unchecked::<PageTable>();
					let l2_index = (virt >> 21) & 0x1FF;
					let l2_entry = &mut l2[l2_index];

					let r = if l2_entry.present() {
						let l1_phys = l2_entry.address();
						let l1 =
							Phys::from_address_unchecked(l1_phys).as_mut_unchecked::<PageTable>();
						let l1_index = (virt >> 12) & 0x1FF;
						let l1_entry = &mut l1[l1_index];

						let r = if l1_entry.present() {
							// NOTE: We DO NOT free the physical frame here.
							// NOTE: We let the caller do that. This is an UNMAP,
							// NOTE: not a FREE.
							let phys = l1_entry.address();
							l1_entry.reset();
							crate::asm::invlpg(virt as *const ());
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

	/// Takes the page table entry at the given index and unmaps it based on the current
	/// depth.
	///
	/// # Safety
	/// Really only meant for mass-unmapping operations. `level` must be the paging
	/// level minus one.
	///
	/// Caller must ensure that ALL pages under the given entry are reclaimable and
	/// that the entry itself is reclaimable, and that none of the reclaimed pages
	/// are still being used.
	#[expect(clippy::only_used_in_recursion)] // false positive
	unsafe fn unmap_and_reclaim_entry<A>(
		&self,
		entry: &mut PageTableEntry,
		alloc: &mut A,
		level: usize,
	) where
		A: Alloc,
	{
		if entry.present() {
			let phys = entry.address();
			// SAFETY(qix-): We know that the physical address is valid.
			let pt = unsafe { Phys::from_address_unchecked(phys).as_mut_unchecked::<PageTable>() };

			if level == 1 {
				for idx in 0..512 {
					let entry = &mut pt[idx];
					if entry.present() {
						alloc.free(entry.address());
					}
				}
			} else {
				for idx in 0..512 {
					self.unmap_and_reclaim_entry(&mut pt[idx], alloc, level - 1);
				}
			}

			// SAFETY(qix-): We know that the physical address is valid.
			alloc.free(phys);
		}
	}

	/// Mirrors (maps the top level page tables) from one address space to another.
	///
	/// Performs no allocation; the mappings, if any, are copied directly from the source
	/// to the destination.
	///
	/// Any existing mappings in the destination are overwritten.
	///
	/// # Safety
	/// Caller must ensure that no overwrites would lead to leaks, and that the mappings
	/// will not cause issues by being mapped between address spaces.
	pub unsafe fn mirror_into(&self, dest: &AddressSpaceHandle, src: &AddressSpaceHandle) {
		let dest_pt = dest.base_phys().as_mut_unchecked::<PageTable>();
		let src_pt = src.base_phys().as_ref_unchecked::<PageTable>();

		for idx in self.valid_range.0..=self.valid_range.1 {
			dest_pt[idx] = src_pt[idx];
		}
	}
}

unsafe impl Segment<AddressSpaceHandle> for &'static AddressSegment {
	// TODO(qix-): Once const trait methods are stabilitized, make this const.
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

	fn apply_user_space_shallow(
		&self,
		destination: &AddressSpaceHandle,
		overlay: &AddressSpaceHandle,
	) -> Result<(), MapError> {
		// SAFETY(qix-): We can assume the destination physical address is a valid page table.
		let dest_pt = unsafe { destination.base_phys().as_mut_unchecked::<PageTable>() };
		// SAFETY(qix-): We can assume the overlay physical address is a valid page table.
		let overlay_pt = unsafe { overlay.base_phys().as_ref_unchecked::<PageTable>() };

		for idx in self.valid_range.0..=self.valid_range.1 {
			if dest_pt[idx].present() {
				return Err(MapError::Exists);
			}
		}

		for idx in self.valid_range.0..=self.valid_range.1 {
			dest_pt[idx] = overlay_pt[idx];
		}

		Ok(())
	}

	unsafe fn unmap_all_without_reclaim(&self, space: &AddressSpaceHandle) {
		let top_level = space.base_phys().as_mut_unchecked::<PageTable>();

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];
			if entry.present() {
				entry.reset();
			}
		}
	}

	unsafe fn unmap_all_and_reclaim_in<A>(&self, space: &AddressSpaceHandle, alloc: &mut A)
	where
		A: Alloc,
	{
		let top_level = space.base_phys().as_mut_unchecked::<PageTable>();

		for idx in self.valid_range.0..=self.valid_range.1 {
			self.unmap_and_reclaim_entry(
				&mut top_level[idx],
				alloc,
				space.paging_level.as_usize() - 1,
			);
		}
	}

	fn provision_as_shared_in<A>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
	) -> Result<(), MapError>
	where
		A: Alloc,
	{
		let top_level = unsafe { space.base_phys().as_mut_unchecked::<PageTable>() };

		for idx in self.valid_range.0..=self.valid_range.1 {
			let entry = &mut top_level[idx];

			if entry.present() {
				return Err(MapError::Exists);
			}

			let frame_phys_addr = alloc.allocate().ok_or(MapError::OutOfMemory)?;
			unsafe {
				Phys::from_address_unchecked(frame_phys_addr)
					.as_mut_unchecked::<PageTable>()
					.reset();
			}
			*entry = self.entry_template.with_address(frame_phys_addr);
		}

		Ok(())
	}

	fn map_in<A>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: Alloc,
	{
		// NOTE(qix-): The current implementation of `entry()` doesn't
		// NOTE(qix-): actually free anyway, so we just proxy to that method.
		self.map_nofree_in(space, alloc, virt, phys)
	}

	fn map_nofree_in<A>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: Alloc,
	{
		let entry = unsafe { self.entry(space, alloc, virt)? };
		if entry.present() {
			return Err(MapError::Exists);
		}

		*entry = self.entry_template.with_address(phys);
		crate::asm::invlpg(virt as *const ());

		Ok(())
	}

	fn unmap_in<A>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		virt: usize,
	) -> Result<u64, UnmapError>
	where
		A: Alloc,
	{
		let phys = unsafe {
			match space.paging_level() {
				PagingLevel::Level4 => self.try_unmap_l4(space, alloc, virt)?,
				PagingLevel::Level5 => self.try_unmap_l5(space, alloc, virt)?,
			}
		};
		phys.ok_or(UnmapError::NotMapped)
	}

	fn remap_in<A>(
		&self,
		space: &AddressSpaceHandle,
		alloc: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<Option<u64>, MapError>
	where
		A: Alloc,
	{
		let entry = unsafe { self.entry(space, alloc, virt)? };
		let old_phys = if entry.present() {
			Some(entry.address())
		} else {
			None
		};

		*entry = self.entry_template.with_address(phys);
		crate::asm::invlpg(virt as *const ());

		Ok(old_phys)
	}
}
