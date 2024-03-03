// NOTE(qix-): DO NOT DERIVE ANY TRAITS FOR TYPES IN THIS FILE.
// NOTE(qix-): DERIVED TRAITS MAY CAUSE ALLOCATITONS, WHICH WOULD
// NOTE(qix-): OTHERWISE CAUSE A MULTITUDE OF PROBLEMS, INCONSISTENT
// NOTE(qix-): BEHAVIOR OR OUTPUTS, ETC.

use crate::{MemoryRegion, MemoryRegionType, PageFrameAllocator};

/// The _first in, last out_ (FILO) page frame allocator is the default [`PageFrameAllocator`]
/// used by the kernel and most bootloaders. Through the use of a [`FiloPageFrameManager`],
/// page frames are brought in and out of a known virtual address location via e.g. a memory
/// map, whereby the last freed page frame physical address is stored in the first bytes of the
/// page. When a page is requested, the allocator first checks the current (stored) page frame
/// address. If it is `u64::MAX`, the allocator allocates the next available page from the
/// memory map. If it is not, the physical paged pointed to by the stored last-free address is
/// brought into virtual memory via the [`FiloPageFrameManager`], the next-last-freed page frame
/// address is read from the first bytes of the page, stored in the allocator's last-free address
/// as the new last-free address, and the page that was just brought in is returned to the
/// requesting kernel code. When a page is freed, the inverse occurs - the page is brought into
/// virtual memory, the current (soon to be previous) last-free value is written to the first
/// few bytes, and the last-free pointer is updated to point to the newly-freed page. This creates
/// a FILO stack of freed pages with no more bookkeeping necessary other than the last-free
/// physical frame pointer.
pub struct FiloPageFrameAllocator<M, R>
where
	M: FiloPageFrameManager,
	R: MemoryRegion + Sized + 'static,
{
	/// The page frame manager that is responsible for bringing in and out
	/// physical pages as needed by the allocator.
	manager: M,
	/// The last-free page frame address.
	last_free: u64,
	/// The memory map used to allocate new system memory.
	memory_regions: &'static [R],
	/// The current memory region index.
	current_region: usize,
	/// The current offset in the current memory region.
	current_offset: u64,
	/// The currently allocated number of bytes.
	used_bytes: u64,
	/// The cached total memory size.
	total_memory: u64,
	/// The cached total usable memory size.
	total_usable_memory: u64,
	/// The cached total unusable memory size.
	total_unusable_memory: u64,
	/// The cached total bad memory size.
	total_bad_memory: Option<u64>,
}

impl<M, R> FiloPageFrameAllocator<M, R>
where
	M: FiloPageFrameManager,
	R: MemoryRegion + Sized + 'static,
{
	/// Creates a new FILO page frame allocator.
	///
	/// # Panics
	/// Panics if `supports_bad_memory` is false, but bad memory
	/// regions (marked as [`MemoryRegionType::Bad`]) are present
	/// in the memory map.
	///
	/// # Safety
	/// This method will either panic or invoke undefined behavior
	/// if memory regions are not:
	///
	/// - Non-overlapping
	/// - Aligned to the page size (4 KiB)
	/// - A multiple of the page size in length
	/// - Sorted by base address
	///
	/// The memory map _may_ have a total byte length of 0 (i.e.
	/// at least one memory region, whereby all memory regions are
	/// either unusable or zero length), but there **must** be one
	/// memory region at the least.
	pub unsafe fn new<const BOOT_IS_USABLE: bool>(
		manager: M,
		memory_regions: &'static [R],
		supports_bad_memory: bool,
	) -> Self {
		let mut total_memory = 0;
		let mut total_usable_memory = 0;
		let mut total_unusable_memory = 0;
		let mut total_bad_memory = if supports_bad_memory { Some(0) } else { None };

		for region in memory_regions {
			total_memory += region.length();
			match region.ty() {
				MemoryRegionType::Usable => total_usable_memory += region.length(),
				MemoryRegionType::Boot => {
					if BOOT_IS_USABLE {
						total_usable_memory += region.length();
					} else {
						total_unusable_memory += region.length();
					}
				}
				MemoryRegionType::Unusable => total_unusable_memory += region.length(),
				MemoryRegionType::Bad => {
					if let Some(total_bad_memory) = total_bad_memory.as_mut() {
						*total_bad_memory += region.length();
					} else {
						panic!("bad memory region provided, but bad memory is not supported");
					}
				}
			}
		}

		Self {
			manager,
			last_free: u64::MAX,
			memory_regions,
			current_region: 0,
			current_offset: 0,
			used_bytes: 0,
			total_memory,
			total_usable_memory,
			total_unusable_memory,
			total_bad_memory,
		}
	}

	/// Allocates a page from, guaranteeing that the page is coming
	/// from the the system regions as opposed to reusing freed
	/// pages. **This method does not use the page frame manager**,
	/// which makes it suitable for early-stage memory table setups
	/// whereby virtual memory is not yet modifiable.
	///
	/// Note that this function _may_ return `None` even if there are
	/// still pages available for allocation via [`PageFrameAllocator::allocate`], since
	/// it only allocates from memory map regions that haven't been
	/// allocated yet.
	pub fn allocate_without_manager(&mut self) -> Option<u64> {
		if self.current_region >= self.memory_regions.len() {
			return None;
		}

		let region = &self.memory_regions[self.current_region];
		let page_frame = region.base() + self.current_offset;
		self.current_offset += 4096;
		self.used_bytes += 4096;

		if self.current_offset >= region.length() {
			self.current_region += 1;
			self.current_offset = 0;
		}

		Some(page_frame)
	}
}

unsafe impl<M, R> PageFrameAllocator for FiloPageFrameAllocator<M, R>
where
	M: FiloPageFrameManager,
	R: MemoryRegion + Sized,
{
	#[inline]
	#[allow(clippy::cast_possible_truncation)]
	unsafe fn allocate(&mut self) -> Option<u64> {
		if self.last_free == u64::MAX {
			self.allocate_without_manager()
		} else {
			// Bring in the last-free page frame.
			let page_frame = self.last_free;
			self.last_free = self.manager.read_u64(page_frame);
			Some(page_frame)
		}
	}

	#[inline]
	unsafe fn free(&mut self, frame: u64) {
		assert_eq!(frame % 4096, 0, "frame is not page-aligned");

		self.manager.write_u64(frame, self.last_free);
		self.last_free = frame;
	}

	#[inline]
	fn used_memory(&self) -> u64 {
		self.used_bytes
	}

	#[inline]
	fn total_unusable_memory(&self) -> u64 {
		self.total_unusable_memory
	}

	#[inline]
	fn total_bad_memory(&self) -> Option<u64> {
		self.total_bad_memory
	}

	#[inline]
	fn total_usable_memory(&self) -> u64 {
		self.total_usable_memory
	}

	#[inline]
	fn total_memory(&self) -> u64 {
		self.total_memory
	}
}

/// A page frame manager is responsible for managing the virtual memory mapping of physical
/// pages as needed by the [`FiloPageFrameAllocator`]. It is responsible for bringing physical
/// pages into virtual memory (usually at a known, fixed address, given that only one page
/// will ever need to be brought in at a time), and for reading/writing values to the first
/// few bytes of the page to indicate the next/previous last-free page frame address as needed
/// by the allocator.
///
/// # Safety
/// Implementors of this trait must ensure that the virtual memory address used to bring in
/// physical pages is safe to use and will not cause any undefined behavior when read from or
/// written to, and that all memory accesses are safe and valid.
pub unsafe trait FiloPageFrameManager {
	/// Brings the given physical page frame into memory and reads the `u64` value
	/// at offset `0`.
	///
	/// # Safety
	/// Implementors of this method must ensure that the virtual memory address used to
	/// bring in physical pages is safe to use and will not cause any undefined behavior
	/// when read from or written to, and that all memory accesses are safe and valid.
	///
	/// Further, implementors must ensure that reads and writes are atomic and volatile,
	/// and that any memory barriers and translation caches (e.g. the TLB) are properly
	/// invalidated and flushed as needed.
	unsafe fn read_u64(&mut self, page_frame: u64) -> u64;

	/// Brings the given physical page frame into memory and writes the `u64` value
	/// at offset `0`.
	///
	/// # Safety
	/// Implementors of this method must ensure that the virtual memory address used to
	/// bring in physical pages is safe to use and will not cause any undefined behavior
	/// when read from or written to, and that all memory accesses are safe and valid.
	///
	/// Further, implementors must ensure that reads and writes are atomic and volatile,
	/// and that any memory barriers and translation caches (e.g. the TLB) are properly
	/// invalidated and flushed as needed.
	unsafe fn write_u64(&mut self, page_frame: u64, value: u64);
}
