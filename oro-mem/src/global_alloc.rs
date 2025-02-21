//! Provides the main global allocator for the Oro kernel.
//!
//! This module is what allows for the use of `alloc`.

use core::{alloc::GlobalAlloc, ptr::NonNull};

use oro_sync::{Lock, TicketMutex};

use crate::{
	pfa::{Alloc, FiloPageFrameAllocator},
	phys::{Phys, PhysAddr},
};

/// Alias for a [`buddy_system_allocator::Heap`] with a pre-defined order.
type Heap = buddy_system_allocator::Heap<64>;

/// The lock-free global PFA.
///
/// We instead use the global `ALLOCATOR` mutex to synchronize access to the heap
/// in order to avoid double-mutex deadlocking.
static PFA: FiloPageFrameAllocator = FiloPageFrameAllocator::new();

/// The global heap allocator for the Oro kernel.
#[cfg_attr(all(not(feature = "std-alloc"), not(test)), global_allocator)]
static ALLOCATOR: GlobalLockedHeap<TicketMutex<Heap>> =
	GlobalLockedHeap(TicketMutex::new(Heap::empty()));

/// Newtype wrapper for the global allocator.
struct GlobalLockedHeap<L>(L)
where
	L: Lock<Target = Heap>;

unsafe impl<L> GlobalAlloc for GlobalLockedHeap<L>
where
	L: Lock<Target = Heap>,
{
	unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
		// NOTE(qix-): For now, we're going to limit the maximum allocation size to 4KiB.
		// NOTE(qix-): This simplifies the implementation of the heap, and can be adjusted
		// NOTE(qix-): later to use the architecture's memory mapping facilities to map larger
		// NOTE(qix-): allocations into a shared memory region.
		debug_assert!(
			layout.size() <= 4096,
			"allocation size too large: {}",
			layout.size()
		);

		let mut heap = self.0.lock();
		if let Ok(ptr) = heap.alloc(layout) {
			ptr.as_ptr()
		} else {
			try_rescue_heap::<L>(&mut heap);

			heap.alloc(layout)
				.map(core::ptr::NonNull::as_ptr)
				.unwrap_or(core::ptr::null_mut())
		}
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
		if let Some(ptr) = NonNull::new(ptr) {
			self.0.lock().dealloc(ptr, layout);
		}
	}
}

/// Attempts to rescue the heap by allocating and mapping new pages.
fn try_rescue_heap<L>(heap: &mut <L as Lock>::Guard<'_>)
where
	L: Lock<Target = Heap>,
{
	// If there are no pages available, we can't do anything;
	// the allocation will fail on return.
	let Some(page) = PFA.allocate() else {
		return;
	};

	// SAFETY: We just allocated this page, so it's safe to use.
	let virt = unsafe { Phys::from_address_unchecked(page) }.virt();

	// SAFETY: It's not documented but under the hood the pointer must be
	// SAFETY: correctly aligned for a `usize` (I don't mean that what
	// SAFETY: we're passing is a `usize`, but the `usize` is a valid pointer
	// SAFETY: value *to* a `usize`). We can guarantee this by design already,
	// SAFETY: but we do an assertion just to 'keep the spaceship flying'.
	oro_macro::assert::aligns_to::<usize, 4096>();
	unsafe {
		heap.add_to_heap(virt, virt + 4096);
	}
}

/// Global page frame allocator proxy type.
///
/// A unit value of this type can be used in all places
/// where a page frame allocator is required in order to
/// safely allocate pages from the global page frame allocator.
pub struct GlobalPfa;

impl GlobalPfa {
	/// Exposes to the global page frame allocator a physical address
	/// range.
	///
	/// Equivalent to calling [`Alloc::free()`] on the `GlobalPfa`
	/// for each aligned page within the range, but is more efficient,
	/// especially on debug builds.
	///
	/// # Safety
	/// The caller **must** ensure that the range is valid, unused,
	/// with no existing references to the memory (and none will be
	/// created after this function is called), and that the memory
	/// is properly mapped into a linear mapping addressible via
	/// the global physical address translator, at the same location
	/// in each of the cores' address spaces.
	pub unsafe fn expose_phys_range(base: u64, length: u64) {
		// Synthesize a lock from the global allocator,
		// effectively synchronizing access to the PFA.
		//
		// This isn't the best way to do this, but it's the
		// most obvious way to do it without introducing a new mutex
		// which could potentially deadlock with the global allocator.
		let lock = ALLOCATOR.0.lock();

		let aligned_base = (base + 4095) & !4095;
		let length = length.saturating_sub(aligned_base - base);

		debug_assert_eq!(aligned_base % 4096, 0);
		debug_assert_eq!(length % 4096, 0);

		// SAFETY: We are in a critical section, which is good enough for the requirements
		// SAFETY: of the dbgutil functions.
		oro_dbgutil::__oro_dbgutil_pfa_will_mass_free(1);
		oro_dbgutil::__oro_dbgutil_pfa_mass_free(aligned_base, aligned_base + length);

		for page in (aligned_base..(aligned_base + length)).step_by(4096) {
			// SAFETY: We're exposing a previously unknown range; the safety requirements are
			// SAFETY: ultimately offloaded to the caller.
			unsafe {
				PFA.free(page);
			}
		}

		// SAFETY: We are in a critical section, which is good enough for the requirements
		// SAFETY: of the dbgutil functions.
		#[cfg(debug_assertions)]
		oro_dbgutil::__oro_dbgutil_pfa_finished_mass_free();

		// Keep the spaceship flying.
		drop(lock);
	}
}

unsafe impl Alloc for GlobalPfa {
	fn allocate(&self) -> Option<u64> {
		// Synthesize a lock from the global allocator,
		// effectively synchronizing access to the PFA.
		//
		// This isn't the best way to do this, but it's the
		// most obvious way to do it without introducing a new mutex
		// which could potentially deadlock with the global allocator.
		PFA.allocate()
	}

	unsafe fn free(&self, frame: u64) {
		// SAFETY: Safety requirements offloaded to caller.
		unsafe {
			PFA.free(frame);
		}
	}
}
