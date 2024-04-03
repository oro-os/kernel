//! Provides a translator mapper for the `x86_64` architecture
//! that constructs page tables for a given address space using
//! physical -> virtual address translation.

// TODO(qix-): The type names aren't very descriptive but I was trying
// TODO(qix-): not to have them conflict with the oro-common trait type
// TODO(qix-): names. They might be renamed in the future.

use crate::{
	mem::{
		layout::{Descriptor, Layout},
		paging_level::PagingLevel,
	},
	PageTable,
};
use core::ptr::from_ref;
use oro_common::{
	mem::{
		AddressSpace, MapError, PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator,
		PrebootAddressSpace, SupervisorAddressSegment, SupervisorAddressSpace, UnmapError,
	},
	unlikely,
};

/// A translator mapper that uses a physical address translator to map virtual addresses
/// to physical addresses for the `x86_64` architecture.
pub struct TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	/// The physical address translator to use for this mapper.
	translator:      P,
	/// Base address of the page table.
	page_table_virt: usize,
	/// The paging level of the CPU
	paging_level:    PagingLevel,
}

unsafe impl<P> AddressSpace for TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	type Layout = Layout;
}

impl<P> SupervisorAddressSpace for TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	type Segment<'a> = TranslatorSupervisorSegment<'a, P> where Self: 'a;

	fn for_supervisor_segment(&self, descriptor: &'static Descriptor) -> Self::Segment<'_> {
		TranslatorSupervisorSegment {
			translator: &self.translator,
			base_table_virt: self.page_table_virt,
			paging_level: self.paging_level,
			descriptor,
		}
	}
}

unsafe impl<P> PrebootAddressSpace<P> for TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	fn new<A>(allocator: &mut A, translator: P) -> Option<Self>
	where
		A: PageFrameAllocate,
	{
		let page_table_phys = allocator.allocate()?;
		let page_table_virt = translator.to_virtual_addr(page_table_phys);

		Some(Self {
			translator,
			page_table_virt,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}
}

/// A single segment in an address space. Segments are used to slice out
/// the address space into logical, well-defined regions for the kernel
/// to interact with.
pub struct TranslatorSupervisorSegment<'a, P>
where
	P: PhysicalAddressTranslator,
{
	/// The physical address translator to use for this segment.
	translator:      &'a P,
	/// Base address of the segment.
	base_table_virt: usize,
	/// The paging level of the CPU
	paging_level:    PagingLevel,
	/// The segment descriptor for this segment
	descriptor:      &'static Descriptor,
}

unsafe impl<'a, P> SupervisorAddressSegment for TranslatorSupervisorSegment<'a, P>
where
	P: PhysicalAddressTranslator,
{
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(MapError::VirtNotAligned);
		}

		{
			let root_index = (virt >> 39) & 0x1FF;
			if unlikely!(
				root_index < self.descriptor.valid_range.0
					|| root_index > self.descriptor.valid_range.1
			) {
				return Err(MapError::VirtOutOfRange);
			}
		}

		let mut current_page_table = self.base_table_virt;

		for level in (1..self.paging_level.as_usize() - 1).rev() {
			let index = (virt >> (12 + level * 9)) & 0x1FF;
			let entry = unsafe { &mut (&mut *(current_page_table as *mut PageTable))[index] };

			current_page_table = if entry.present() {
				self.translator.to_virtual_addr(entry.address())
			} else {
				let frame_phys_addr = allocator.allocate().ok_or(MapError::OutOfMemory)?;
				*entry = self.descriptor.entry_template.with_address(frame_phys_addr);
				self.translator.to_virtual_addr(frame_phys_addr)
			};
		}

		let entry =
			unsafe { &mut (&mut *(current_page_table as *mut PageTable))[(virt >> 12) & 0x1FF] };
		if entry.present() {
			return Err(MapError::Exists);
		}

		*entry = self.descriptor.entry_template.with_address(phys);

		Ok(())
	}

	fn unmap<A>(&mut self, allocator: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(UnmapError::VirtNotAligned);
		}

		{
			let root_index = (virt >> 39) & 0x1FF;
			if unlikely!(
				root_index < self.descriptor.valid_range.0
					|| root_index > self.descriptor.valid_range.1
			) {
				return Err(UnmapError::VirtOutOfRange);
			}
		}

		// Note that we don't use a loop here so that we can unmap the entries
		// if they're empty.
		let l4 = unsafe { &mut *(self.base_table_virt as *mut PageTable) };
		let l4_entry = &mut l4[(virt >> 39) & 0x1FF];
		if !l4_entry.present() {
			return Err(UnmapError::NotMapped);
		}

		let l3 = unsafe {
			&mut *(self.translator.to_virtual_addr(l4_entry.address()) as *mut PageTable)
		};
		let l3_entry = &mut l3[(virt >> 30) & 0x1FF];
		if !l3_entry.present() {
			return Err(UnmapError::NotMapped);
		}

		let l2 = unsafe {
			&mut *(self.translator.to_virtual_addr(l3_entry.address()) as *mut PageTable)
		};
		let l2_entry = &mut l2[(virt >> 21) & 0x1FF];
		if !l2_entry.present() {
			return Err(UnmapError::NotMapped);
		}

		let l1 = unsafe {
			&mut *(self.translator.to_virtual_addr(l2_entry.address()) as *mut PageTable)
		};
		let l1_entry = &mut l1[(virt >> 12) & 0x1FF];
		if !l1_entry.present() {
			return Err(UnmapError::NotMapped);
		}

		let phys_addr = if self.paging_level == PagingLevel::Level5 {
			let l0 = unsafe {
				&mut *(self.translator.to_virtual_addr(l1_entry.address()) as *mut PageTable)
			};
			let l0_entry = &mut l0[(virt >> 3) & 0x1FF];
			if !l0_entry.present() {
				return Err(UnmapError::NotMapped);
			}

			// Important! Do not actually free the leaf physical address.
			let phys_addr = l0_entry.address();
			l0_entry.reset();
			crate::asm::invlpg(virt);

			if l0.empty() {
				unsafe {
					allocator.free(l1_entry.address());
				}
				l1_entry.reset();
				crate::asm::invlpg(from_ref(&l0) as usize);
			}

			phys_addr
		} else {
			let phys_addr = l1_entry.address();
			l1_entry.reset();
			crate::asm::invlpg(virt);
			phys_addr
		};

		if l1.empty() {
			unsafe {
				allocator.free(l2_entry.address());
			}
			l2_entry.reset();
			crate::asm::invlpg(from_ref(&l1) as usize);
		}

		if l2.empty() {
			unsafe {
				allocator.free(l3_entry.address());
			}
			l3_entry.reset();
			crate::asm::invlpg(from_ref(&l2) as usize);
		}

		if l3.empty() {
			unsafe {
				allocator.free(l4_entry.address());
			}
			l4_entry.reset();
			crate::asm::invlpg(from_ref(&l3) as usize);
		}

		Ok(phys_addr)
	}
}
