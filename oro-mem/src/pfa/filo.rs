//! Provides the types for the First In, Last Out (FILO) page frame allocator,
//! whereby page frames form a linked list of free pages. See [`FiloPageFrameAllocator`]
//! for more information.

use crate::{
	pfa::alloc::Alloc,
	phys::{Phys, PhysAddr},
};

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
