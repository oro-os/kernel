//! Allocator implementation for the Oro kernel.

use core::ptr::NonNull;

use crate::{buddy_system::Heap, lock::Mutex, syscall_get};

#[doc(hidden)]
const ORDER: usize = 32;

/// A heap allocator using Oro memory tokens.
///
/// This allocator is a simple wrapper around the Oro memory token system
/// and a buddy allocator. It implements the allocator API from the `alloc`
/// crate.
pub struct HeapAllocator(Mutex<HeapAllocatorInner>);

/// Inner state for the [`HeapAllocator`].
struct HeapAllocatorInner {
	/// The buddy system heap.
	heap: Heap<ORDER>,
	/// The current base cursor for heap page allocations.
	///
	/// 0 means that the heap top has not been set yet.
	base: u64,
}

impl HeapAllocator {
	/// Creates a new [`HeapAllocator`] instance.
	#[must_use]
	pub const fn new() -> Self {
		Self(Mutex::new(HeapAllocatorInner {
			heap: Heap::new(),
			base: 0,
		}))
	}
}

unsafe impl core::alloc::GlobalAlloc for HeapAllocator {
	unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
		// Attempt to allocate from the heap.
		let mut inner = self.0.lock();

		if let Ok(ptr) = inner.heap.alloc(layout) {
			ptr.as_ptr()
		} else {
			// Try to rescue.
			let num_pages = (layout.size().saturating_add(4095)) >> 12;
			let num_pages = num_pages as u64;

			// SAFETY: Allocating a token is always safe.
			let Ok(token) = (unsafe {
				crate::syscall_get!(
					crate::id::iface::KERNEL_PAGE_ALLOC_V0,
					crate::id::iface::KERNEL_PAGE_ALLOC_V0,
					num_pages,
					crate::key!("4kib"),
				)
			}) else {
				// Nothing we can do.
				return core::ptr::null_mut();
			};

			if inner.base == 0 {
				// First allocation; set the base.
				inner.base = heap_top();
			}

			// We've allocated the token; now map it.
			// We do this until we don't conflict.
			loop {
				inner.base = if let Some(base) = inner.base.checked_sub(num_pages << 12) {
					base
				} else {
					// We've run out of heap space. Best-effort forget the token.
					// SAFETY: We just allocated the token; it's safe to forget it.
					let _ = unsafe {
						crate::syscall_set!(
							crate::id::iface::KERNEL_MEM_TOKEN_V0,
							crate::id::iface::KERNEL_MEM_TOKEN_V0,
							token,
							crate::key!("forget"),
							1,
						)
					};

					return core::ptr::null_mut();
				};

				debug_assert!(
					inner.base.is_multiple_of(4096),
					"inner.base is not page-aligned"
				);

				// SAFETY: We've checked that the base area is unused.
				let result = unsafe {
					crate::syscall_set!(
						crate::id::iface::KERNEL_MEM_TOKEN_V0,
						crate::id::iface::KERNEL_MEM_TOKEN_V0,
						token,
						crate::key!("base"),
						inner.base,
					)
				};

				// TODO(qix-): Gets around the need for a nightly feature to use in a match arm.
				// TODO(qix-): Inline when `inline_const_pat` is stable.
				#[doc(hidden)]
				const CONFLICT: u64 = crate::key!("conflict");

				match result {
					Ok(()) => break,
					Err((crate::syscall::Error::InterfaceError, CONFLICT)) => {
						// Conflict; try again.
					}
					Err(_) => {
						// Something went wrong; nothing we can do. Best effort
						// forget the token.
						// SAFETY: We just allocated the token; it's safe to forget it.
						let _ = unsafe {
							crate::syscall_set!(
								crate::id::iface::KERNEL_MEM_TOKEN_V0,
								crate::id::iface::KERNEL_MEM_TOKEN_V0,
								token,
								crate::key!("forget"),
								1,
							)
						};

						return core::ptr::null_mut();
					}
				}
			}

			// We've successfully mapped the token; now report it to the heap...
			let start = inner.base as usize;
			let end = inner.base as usize + (num_pages << 12) as usize;
			// SAFETY: We've checked that the area is safe to add to the heap.
			unsafe {
				inner.heap.add_to_heap(start, end);
			}

			// ... and attempt the allocation again.
			let ptr = inner
				.heap
				.alloc(layout)
				.map(|p| p.as_ptr())
				.unwrap_or_default();

			// Keep the space ship flying.
			drop(inner);

			ptr
		}
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
		// SAFETY: The safety requirements are offloaded to the caller.
		unsafe {
			if let Some(ptr) = NonNull::new(ptr) {
				self.0.lock().heap.dealloc(ptr, layout);
			}
		}
	}
}

/// The top address (exclusive; one byte higher past the end)
/// of the heap. The heap's pages grow downwards from this address.
///
/// # Performance
/// This is a "cold" function, and should be used sparingly.
/// It incurs a syscall and a number of runtime assertions.
///
/// # Panics
/// Panics if the heap top syscall fails, or if the kernel reports
/// `u64::MAX` as the heap top (it should never do this).
#[must_use]
pub fn heap_top() -> u64 {
	// SAFETY: This is a query; it's always safe.
	let high_byte = unsafe {
		syscall_get!(
			crate::id::iface::KERNEL_ADDR_LAYOUT_V0,
			crate::id::iface::KERNEL_ADDR_LAYOUT_V0,
			crate::key!("module"),
			crate::key!("end"),
		)
		.expect("failed to get heap top")
	};

	let high_byte = high_byte.wrapping_add(1);
	assert_ne!(
		high_byte, 0,
		"heap top is at highest address; allocator will not work"
	);

	assert!(
		high_byte % 4096 == 0,
		"heap top is non-page-aligned address"
	);

	high_byte
}
