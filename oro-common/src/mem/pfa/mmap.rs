use crate::mem::{MemoryRegion, MemoryRegionType, PageFrameAllocate};

/// A low-level system allocator that allocates frames directly from
/// a memory map.
///
/// This allocator is not safe to use in a multiprocessor context;
/// all accesses must be properly synchronized.
pub struct MmapPageFrameAllocator<R, I, const BOOT_IS_USABLE: bool>
where
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	memory_regions: I,
	current_region: Option<R>,
	current_offset: u64,
}

impl<R, I, const BOOT_IS_USABLE: bool> MmapPageFrameAllocator<R, I, BOOT_IS_USABLE>
where
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	/// Creates a new memory map page frame allocator.
	///
	/// # Safety
	/// The memory map must be valid and usable.
	pub unsafe fn new(memory_regions: I) -> Self {
		Self {
			memory_regions,
			current_region: None,
			current_offset: 0,
		}
	}

	/// Consumes this allocator and returns the remaining memory regions:
	///
	/// - The unconsumed memory regions iterator.
	/// - The current memory region, if any.
	/// - The current offset into the current memory region,
	///   or _an undefined value_ if there is none.
	pub fn into_inner(self) -> (I, Option<R>, u64) {
		(
			self.memory_regions,
			self.current_region,
			self.current_offset,
		)
	}
}

unsafe impl<R, I, const BOOT_IS_USABLE: bool> PageFrameAllocate
	for MmapPageFrameAllocator<R, I, BOOT_IS_USABLE>
where
	R: MemoryRegion + Sized + 'static,
	I: Iterator<Item = R>,
{
	unsafe fn allocate(&mut self) -> Option<u64> {
		while self.current_region.is_none() {
			let next_region = self.memory_regions.next().map(|r| r.aligned(4096))?;

			match next_region.ty() {
				MemoryRegionType::Bad | MemoryRegionType::Unusable => continue,
				MemoryRegionType::Usable => {}
				MemoryRegionType::Boot => {
					if !BOOT_IS_USABLE {
						continue;
					}
				}
			}

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

		Some(page_frame)
	}
}
