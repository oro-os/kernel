//! Provides a translator mapper for the x86_64 architecture
//! that constructs page tables for a given address space using
//! physical -> virtual address translation.

// TODO(qix-): The type names aren't very descriptive but I was trying
// TODO(qix-): not to have them conflict with the oro-common trait type
// TODO(qix-): names. They might be renamed in the future.

use crate::mem::{
	layout::{Descriptor, Layout},
	paging::PageTable,
	paging_level::PagingLevel,
};
use core::{fmt, ptr::from_ref};
use oro_common::{
	mem::{
		AddressSpace, CloneToken, MapError, PageFrameAllocate, PageFrameFree,
		PhysicalAddressTranslator, PrebootAddressSpace, SupervisorAddressSegment,
		SupervisorAddressSpace, UnmapError,
	},
	unlikely,
};

/// A translator mapper that uses a physical address translator to map virtual addresses
/// to physical addresses for the x86_64 architecture.
#[derive(Clone)]
pub struct TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	/// The physical address translator to use for this mapper.
	translator:      P,
	/// Physical address of the root page table entry.
	page_table_phys: u64,
	/// Base address of the page table.
	page_table_virt: usize,
	/// The paging level of the CPU
	paging_level:    PagingLevel,
}

impl<P> fmt::Debug for TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("TranslatorMapper")
			.field("page_table_phys", &self.page_table_phys)
			.field("page_table_virt", &self.page_table_virt)
			.field("paging_level", &self.paging_level)
			.field_with("page_table", |f| {
				let table = unsafe { &*(self.page_table_virt as *const PageTable) };
				table.fmt(f)
			})
			.finish_non_exhaustive()
	}
}

impl<P> TranslatorMapper<P>
where
	P: PhysicalAddressTranslator,
{
	/// Constructs a new translator mapper using the current CPU's paging
	/// table (CR3) and the given physical address translator.
	///
	/// # Safety
	/// Calls to this function must not overlap lifetime-wise with any other
	/// address space instances that use current CPU paging tables.
	pub unsafe fn get_current(translator: P) -> Self {
		let page_table_phys = crate::asm::cr3();
		let page_table_virt = translator.to_virtual_addr(page_table_phys);

		Self {
			translator,
			page_table_phys,
			page_table_virt,
			paging_level: PagingLevel::current_from_cpu(),
		}
	}

	/// Returns the x86_64-specific stubs segment for this address space.
	pub fn stubs(&self) -> <Self as SupervisorAddressSpace>::Segment<'_> {
		self.for_supervisor_segment(Layout::stubs())
	}

	/// Returns the x86_64-specific kernel stack segment for this address space.
	pub fn kernel_stack(&self) -> <Self as SupervisorAddressSpace>::Segment<'_> {
		self.for_supervisor_segment(Layout::kernel_stack())
	}

	/// Returns the physical address of the root page table entry.
	pub fn page_table_phys(&self) -> u64 {
		self.page_table_phys
	}

	/// Clones this mapper into a new instance, using the same layout but
	/// cloning the top-level page table.
	///
	/// # Panics
	/// May panic if allocation fails.
	pub fn clone_top_level<A>(&self, alloc: &mut A) -> Self
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		let page_table_phys = alloc
			.allocate()
			.expect("failed to allocate page for top-level table");
		let page_table_virt = self.translator.to_virtual_addr(page_table_phys);

		unsafe {
			core::ptr::copy_nonoverlapping(
				self.page_table_virt as *const u8,
				page_table_virt as *mut u8,
				4096,
			);
		}

		Self {
			translator: self.translator.clone(),
			page_table_phys,
			page_table_virt,
			paging_level: self.paging_level,
		}
	}
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
	// We just use ourselves since we're the only address space.
	type CloneToken = PrebootAddressSpaceClone<P>;

	fn new<A>(allocator: &mut A, translator: P) -> Option<Self>
	where
		A: PageFrameAllocate,
	{
		let page_table_phys = allocator.allocate()?;
		let page_table_virt = translator.to_virtual_addr(page_table_phys);

		unsafe {
			core::slice::from_raw_parts_mut(page_table_virt as *mut u8, 4096).fill(0);
		}

		Some(Self {
			translator,
			page_table_phys,
			page_table_virt,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn clone_token(&self) -> Self::CloneToken {
		PrebootAddressSpaceClone {
			translator:      self.translator.clone(),
			page_table_phys: self.page_table_phys,
		}
	}

	fn from_token<A>(token: Self::CloneToken, _alloc: &mut A) -> Self
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		let page_table_virt = token.translator.to_virtual_addr(token.page_table_phys);

		Self {
			translator: token.translator,
			page_table_phys: token.page_table_phys,
			page_table_virt,
			paging_level: PagingLevel::current_from_cpu(),
		}
	}
}

/// A [`CloneToken`] for a preboot address space.
#[repr(C, align(16))]
#[derive(Clone)]
pub struct PrebootAddressSpaceClone<P>
where
	P: PhysicalAddressTranslator,
{
	/// Clone of the physical address translator from the originating address space.
	translator:      P,
	/// Physical address of the root page table entry of the originating address space.
	page_table_phys: u64,
}

impl<P> CloneToken for PrebootAddressSpaceClone<P> where P: PhysicalAddressTranslator {}

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

impl<'a, P> TranslatorSupervisorSegment<'a, P>
where
	P: PhysicalAddressTranslator,
{
	fn maybe_remap<A, const REMAP: bool>(
		&mut self,
		allocator: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
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

		for level in (1..self.paging_level.as_usize()).rev() {
			let index = (virt >> (12 + level * 9)) & 0x1FF;
			let entry = unsafe { &mut (&mut *(current_page_table as *mut PageTable))[index] };

			current_page_table = if entry.present() {
				self.translator.to_virtual_addr(entry.address())
			} else {
				let frame_phys_addr = allocator.allocate().ok_or(MapError::OutOfMemory)?;
				*entry = self.descriptor.entry_template.with_address(frame_phys_addr);
				let frame_virt_addr = self.translator.to_virtual_addr(frame_phys_addr);
				crate::asm::invlpg(frame_virt_addr);

				unsafe {
					core::slice::from_raw_parts_mut(frame_virt_addr as *mut u8, 4096).fill(0);
				}
				frame_virt_addr
			};
		}

		let entry =
			unsafe { &mut (&mut *(current_page_table as *mut PageTable))[(virt >> 12) & 0x1FF] };

		if !REMAP && entry.present() {
			return Err(MapError::Exists);
		}

		*entry = self.descriptor.entry_template.with_address(phys);
		crate::asm::invlpg(virt);

		Ok(())
	}
}

unsafe impl<'a, P> SupervisorAddressSegment for TranslatorSupervisorSegment<'a, P>
where
	P: PhysicalAddressTranslator,
{
	#[inline]
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		self.maybe_remap::<A, false>(allocator, virt, phys)
	}

	#[inline]
	fn remap<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		self.maybe_remap::<A, true>(allocator, virt, phys)
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
