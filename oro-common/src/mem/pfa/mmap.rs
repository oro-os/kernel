//! Provides a low-level page frame allocator based on a memory map
//! iterator. See [`MmapPageFrameAllocator`] for more information.

use crate::{
	mem::{MemoryRegion, PageFrameAllocate},
	Arch,
};

/// A low-level system allocator that allocates frames directly from
/// a memory map.
///
/// This allocator is not safe to use in a multiprocessor context;
/// all accesses must be properly synchronized.
pub struct MmapPageFrameAllocator<A, R, I>
where
	A: Arch,
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	/// An iterator over **usable** memory regions.
	memory_regions: I,
	/// The current memory region from which pages
	/// are being allocated, if any.
	current_region: Option<R>,
	/// The current offset into the current memory region,
	/// from which the next page frame will be allocated.
	current_offset: u64,
	/// The architecture type.
	_arch:          core::marker::PhantomData<A>,
}

impl<A, R, I> MmapPageFrameAllocator<A, R, I>
where
	A: Arch,
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	/// Creates a new memory map page frame allocator.
	///
	/// # Safety
	/// The memory map iterator **MUST** be filtered for
	/// only usable regions of memory. The allocator
	/// will **indiscriminantly** allocate pages from
	/// any region passed to it via the iterator.
	pub unsafe fn new(memory_regions: I) -> Self {
		Self {
			memory_regions,
			current_region: None,
			current_offset: 0,
			_arch: core::marker::PhantomData,
		}
	}

	/// Consumes this allocator and returns the remaining memory regions:
	///
	/// - The unconsumed memory regions iterator.
	/// - The current memory region, if any.
	/// - The current offset into the current memory region,
	///   or _an undefined value_ if there is none.
	///
	/// Note that **all consumed memory regions**, including the one
	/// returned at tuple position 2, **have been aligned** via
	/// [`MemoryRegion::aligned()`]. Be sure to take that into
	/// account when considering what memory has or has not been
	/// consumed.
	#[cold]
	pub fn into_inner(self) -> (I, Option<R>, u64) {
		A::strong_memory_barrier();

		(
			self.memory_regions,
			self.current_region,
			self.current_offset,
		)
	}
}

unsafe impl<A, R, I> PageFrameAllocate for MmapPageFrameAllocator<A, R, I>
where
	A: Arch,
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	unsafe fn allocate(&mut self) -> Option<u64> {
		while self.current_region.is_none() {
			let next_region = self.memory_regions.next().map(|r| r.aligned(4096))?;

			if next_region.length() >= 4096 {
				self.current_region = Some(next_region);
				self.current_offset = 0;
				break;
			}
		}

		let region = self.current_region.as_ref().unwrap();
		let page_frame = region.base() + self.current_offset;

		self.current_offset += 4096;

		if self.current_offset >= region.length() {
			self.current_region = None;
		}

		A::strong_memory_barrier();

		Some(page_frame)
	}
}
