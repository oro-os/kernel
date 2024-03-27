use crate::mem::{AllocatorStatsTracker, PageFrameAllocate, PageFrameFree};

/// The _first in, last out_ (FILO) page frame allocator is the default page frame allocator
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
pub struct FiloPageFrameAllocator<A, M>
where
	A: PageFrameAllocate,
	M: FiloPageFrameManager,
{
	/// The underlying page frame allocator that provides
	/// system frames from e.g. a memory map.
	frame_allocator: A,
	/// The manager responsible for bringing in and out
	/// physical pages from virtual memory.
	manager: M,
	/// The last-free page frame address.
	last_free: u64,
	/// The stats tracker this allocator uses.
	tracker: AllocatorStatsTracker,
}

impl<A, M> FiloPageFrameAllocator<A, M>
where
	A: PageFrameAllocate,
	M: FiloPageFrameManager,
{
	/// Creates a new FILO page frame allocator.
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
	pub unsafe fn new(
		frame_allocator: A,
		manager: M,
		stats_tracker: AllocatorStatsTracker,
	) -> Self {
		Self {
			frame_allocator,
			manager,
			last_free: u64::MAX,
			tracker: stats_tracker,
		}
	}

	/// Gets a reference to the stats tracker used by this allocator.
	#[inline]
	pub fn stats(&self) -> &AllocatorStatsTracker {
		&self.tracker
	}
}

unsafe impl<A, M> PageFrameAllocate for FiloPageFrameAllocator<A, M>
where
	A: PageFrameAllocate,
	M: FiloPageFrameManager,
{
	#[inline]
	#[allow(clippy::cast_possible_truncation)]
	unsafe fn allocate(&mut self) -> Option<u64> {
		let page_frame = if self.last_free == u64::MAX {
			// Allocate from the underlying memory map allocator
			self.frame_allocator.allocate()
		} else {
			// Bring in the last-free page frame.
			let page_frame = self.last_free;
			self.last_free = self.manager.read_u64(page_frame);
			Some(page_frame)
		};

		if page_frame.is_some() {
			self.tracker.add_used_bytes(4096);
		}

		page_frame
	}
}

unsafe impl<A, M> PageFrameFree for FiloPageFrameAllocator<A, M>
where
	A: PageFrameAllocate,
	M: FiloPageFrameManager,
{
	#[inline]
	unsafe fn free(&mut self, frame: u64) {
		assert_eq!(frame % 4096, 0, "frame is not page-aligned");

		self.manager.write_u64(frame, self.last_free);
		self.last_free = frame;
		self.tracker.sub_used_bytes(4096);
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
