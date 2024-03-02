/// A page frame allocator allocates physical memory in units of "page frames".
/// A page frame is a contiguous block of physical memory that is a multiple of
/// the requested page size (e.g. 4 KiB).
///
/// The page frame allocator is responsible for tracking and re-using freed
/// page frames, and for providing new page frames to the kernel when requested.
/// It is also responsible for indicating when the system has run out of memory,
/// as well as providing statistics about the memory usage of the system.
///
/// # Safety
/// The page frame allocator must ensure that all memory accesses are safe and valid
/// during any bookkeeping operations.
///
/// Further, it must ensure that page frame addresses are properly aligned and that
/// no overlapping frames are allocated.
pub unsafe trait PageFrameAllocator {
	/// Allocates a new page frame, returning the physical address of the page frame
	/// that was allocated. If `None` is returned, the system is out of memory.
	///
	fn allocate(&mut self) -> Option<usize>;

	/// Frees a page frame.
	///
	/// # Panics
	/// Implementors **must** panic if the passed frame address is not page-aligned.
	fn free(&mut self, frame: usize);

	/// Gets the number of bytes of memory that are currently in use by the system.
	fn used_memory(&self) -> usize;

	/// Gets the number of bytes of memory that are currently free and available to the system.
	/// This does not include unusable memory regions.
	#[inline]
	fn free_memory(&self) -> usize {
		self.total_usable_memory() - self.used_memory()
	}

	/// Gets the number of bytes of memory in the system that are unusable.
	/// This **does not** include bad memory.
	fn total_unusable_memory(&self) -> usize;

	/// Gets the number of bytes of "bad" memory in the system.
	/// This is **not** simply unusable memory, but memory explicitly marked
	/// as "bad" by the bootloader. Returns `None` if the bootloader does not
	/// provide this information.
	fn total_bad_memory(&self) -> Option<usize>;

	/// Gets the total amount of memory, including usable, unusable, and bad memory.
	fn total_memory(&self) -> usize {
		self.total_usable_memory()
			+ self.total_unusable_memory()
			+ self.total_bad_memory().unwrap_or(0)
	}

	/// Gets the total number of bytes of memory that are usable to the system.
	fn total_usable_memory(&self) -> usize;
}
