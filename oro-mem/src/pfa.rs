//! Page frame allocator traits and implementations.

use core::sync::atomic::{
	AtomicU64,
	Ordering::{Acquire, Relaxed, Release},
};

use crate::phys::{Phys, PhysAddr};

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
	fn allocate(&self) -> Option<u64>;

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
	unsafe fn free(&self, frame: u64);
}

/// First in, last out (FILO) page frame allocator.
///
/// The _first in, last out_ (FILO) page frame allocator is the
/// default page frame allocator used by the kernel and most
/// bootloaders. Through the use of the global physical address translator,
/// page frames are translated to virtual addresses and read/written to,
/// whereby the last freed page
/// frame physical address is stored in the first bytes of the
/// page.
///
/// When a page is requested, the allocator first checks the
/// current (stored) page frame address. If it is `u64::MAX`, the
/// allocator is out of memory. If it is not, the physical page
/// pointed to by the stored last-free address is ultimately used as the result, the
/// next-last-freed page frame address is read from the first
/// bytes of the page, stored in the allocator's last-free
/// address as the new last-free address, and page's physical address is returned.
///
/// When a page is freed, the inverse occurs - the page is
/// translated into virtual memory, the current (soon to be
/// previous) last-free value is written to the first few bytes,
/// and the last-free pointer is updated to point to the
/// newly-freed page. This creates a FILO stack of freed pages
/// with no more bookkeeping necessary other than the last-free
/// physical frame pointer.
pub struct FiloPageFrameAllocator {
	/// The last-free page frame address.
	last_free: AtomicU64,
}

impl FiloPageFrameAllocator {
	/// Creates a new FILO page frame allocator.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			last_free: AtomicU64::new(u64::MAX),
		}
	}

	/// Creates a new FILO page frame allocator with the given
	/// last-free page frame address.
	#[inline]
	#[must_use]
	pub fn with_last_free(last_free: u64) -> Self {
		Self {
			last_free: AtomicU64::new(last_free),
		}
	}
}

unsafe impl Alloc for FiloPageFrameAllocator {
	fn allocate(&self) -> Option<u64> {
		let mut loaded = self.last_free.load(Acquire);
		loop {
			if loaded == u64::MAX {
				// We're out of memory
				return None;
			}

			// SAFETY: This might read garbage data. That's fine;
			// SAFETY: in the event that it does, it also means
			// SAFETY: that `self.last_free` was changed, and the
			// SAFETY: CXC will just try again.
			let new_free = unsafe {
				Phys::from_address_unchecked(loaded)
					.as_ptr_unchecked::<u64>()
					.read_volatile()
			};

			if let Err(err) = self
				.last_free
				.compare_exchange(loaded, new_free, Release, Relaxed)
			{
				loaded = err;
			} else {
				oro_dbgutil::__oro_dbgutil_pfa_alloc(loaded);
				return Some(loaded);
			}
		}
	}

	unsafe fn free(&self, frame: u64) {
		assert_eq!(frame % 4096, 0, "frame is not page-aligned");

		let mut loaded = self.last_free.load(Acquire);
		loop {
			// Write first; we might have to write multiple times.
			// SAFETY: We assume control of this frame; the caller must
			// SAFETY: ensure that's the case.
			unsafe {
				Phys::from_address_unchecked(frame)
					.as_mut_ptr_unchecked::<u64>()
					.write_volatile(loaded);
			}

			if let Err(err) = self
				.last_free
				.compare_exchange(loaded, frame, Release, Relaxed)
			{
				loaded = err;
			} else {
				oro_dbgutil::__oro_dbgutil_pfa_free(frame);
				return;
			}
		}
	}
}
