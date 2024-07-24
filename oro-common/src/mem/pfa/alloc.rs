//! Provides allocate/free traits for all allocators.

/// A page frame allocator allocates physical memory in units of "page frames".
/// A page frame is a contiguous block of physical memory that is a multiple of
/// the requested page size (e.g. 4 KiB).
///
/// Page allocators that support freeing page frames should also implement the
/// [`PageFrameFree`] trait.
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
/// Any and all bookkeeping operations must be safe.
pub unsafe trait PageFrameAllocate {
	/// Allocates a new page frame, returning the physical address of the page frame
	/// that was allocated. If `None` is returned, the system is out of memory.
	fn allocate(&mut self) -> Option<u64>;
}

/// A page frame allocator that supports freeing page frames.
///
/// # Safety
/// Implementations of this trait must ensure that all memory accesses are safe and valid
/// during any bookkeeping operations.
///
/// Implementations **must** panic if the passed frame address is not page-aligned.
///
/// Any and all bookkeeping operations must be safe.
pub unsafe trait PageFrameFree: PageFrameAllocate {
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
	unsafe fn free(&mut self, frame: u64);
}
