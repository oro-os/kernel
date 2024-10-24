//! Page frame allocator traits and implementations.

use oro_sync::Lock;

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
	#[expect(clippy::missing_safety_doc)]
	unsafe fn free(&self, frame: u64);
}

unsafe impl<L> GlobalPfa for L
where
	L: Lock,
	<L as Lock>::Target: Alloc,
{
	fn allocate(&self) -> Option<u64> {
		let mut lock = self.lock();
		let r = lock.allocate();
		drop(lock);
		r
	}

	unsafe fn free(&self, frame: u64) {
		let mut lock = self.lock();
		lock.free(frame);
		drop(lock);
	}
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
	last_free: u64,
}

impl FiloPageFrameAllocator {
	/// Creates a new FILO page frame allocator.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			last_free: u64::MAX,
		}
	}

	/// Creates a new FILO page frame allocator with the given
	/// last-free page frame address.
	#[inline]
	#[must_use]
	pub fn with_last_free(last_free: u64) -> Self {
		Self { last_free }
	}

	/// Returns the last-free page frame address.
	#[inline]
	#[must_use]
	pub fn last_free(&self) -> u64 {
		self.last_free
	}
}

unsafe impl Alloc for FiloPageFrameAllocator {
	fn allocate(&mut self) -> Option<u64> {
		if self.last_free == u64::MAX {
			// We're out of memory
			None
		} else {
			// Bring in the last-free page frame.
			let page_frame = self.last_free;
			self.last_free = unsafe {
				Phys::from_address_unchecked(page_frame)
					.as_ptr_unchecked::<u64>()
					.read_volatile()
			};
			#[cfg(debug_assertions)]
			oro_dbgutil::__oro_dbgutil_pfa_alloc(page_frame);
			Some(page_frame)
		}
	}

	unsafe fn free(&mut self, frame: u64) {
		assert_eq!(frame % 4096, 0, "frame is not page-aligned");
		#[cfg(debug_assertions)]
		oro_dbgutil::__oro_dbgutil_pfa_free(frame);
		unsafe {
			Phys::from_address_unchecked(frame)
				.as_mut_ptr_unchecked::<u64>()
				.write_volatile(self.last_free);
		}
		self.last_free = frame;
	}
}
