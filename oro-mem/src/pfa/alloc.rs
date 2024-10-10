//! Provides allocate/free traits for all allocators.

/// A page frame allocator allocates physical memory in units of "page frames".
/// A page frame is a contiguous block of physical memory that is a multiple of
/// the requested page size (e.g. 4 KiB).
///
/// Consumers of this trait must ensure proper synchronization if the allocator
/// is shared between multiple processors. Implementations **should not** provide any
/// thread safety.
///
/// # Safety
/// Implementations **must** ensure that the returned frame address
///
/// - is page-aligned.
/// - is not already in use.
/// - is not in a reserved, bad, or unusable memory region.
/// - not overlapping with any other allocated frame.
///
/// Any and all bookkeeping operations must be safe and **MUST NOT panic**.
pub unsafe trait Alloc {
	/// Allocates a new page frame, returning the physical address of the page frame
	/// that was allocated. If `None` is returned, the system is out of memory.
	fn allocate(&mut self) -> Option<u64>;

	/// Frees a page frame.
	///
	/// # Safety
	/// The following **must absolutely remain true**:
	///
	/// 1. Callers **must** ensure the passed frame address is valid and allocated, not in active
	///    use, and is not already freed. Implementors are under no obligation to ensure this.
	///
	/// 2. Callers **must** ensure the passed frame address is not in a reserved or unusable
	///    memory region.
	///
	/// 3. Callers **must** ensure the frame is page-aligned.
	unsafe fn free(&mut self, frame: u64);
}

/// A global page frame allocator. Identical to [`Alloc`]
/// except that methods are not mutable.
///
/// # Safety
/// See [`Alloc`] for safety requirements.
pub unsafe trait GlobalPfa {
	/// See [`Alloc::allocate`].
	fn allocate(&self) -> Option<u64>;

	/// See [`Alloc::free`].
	unsafe fn free(&self, frame: u64);
}

unsafe impl<T: GlobalPfa> Alloc for T {
	fn allocate(&mut self) -> Option<u64> {
		GlobalPfa::allocate(self)
	}

	unsafe fn free(&mut self, frame: u64) {
		GlobalPfa::free(self, frame)
	}
}
