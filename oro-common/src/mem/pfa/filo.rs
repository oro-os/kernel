//! Provides the types for the First In, Last Out (FILO) page frame allocator,
//! whereby page frames form a linked list of free pages. See [`FiloPageFrameAllocator`]
//! for more information.

use crate::mem::{PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator};

/// The _first in, last out_ (FILO) page frame allocator is the
/// default page frame allocator used by the kernel and most
/// bootloaders. Through the use of a [`FiloPageFrameManager`],
/// page frames are brought in and out of a known virtual address
/// location via e.g. a memory map, whereby the last freed page
/// frame physical address is stored in the first bytes of the
/// page.
///
/// When a page is requested, the allocator first checks the
/// current (stored) page frame address. If it is `u64::MAX`, the
/// allocator is out of memory. If it is not, the physical page
/// pointed to by the stored last-free address is brought into
/// virtual memory via the [`FiloPageFrameManager`], the
/// next-last-freed page frame address is read from the first
/// bytes of the page, stored in the allocator's last-free
/// address as the new last-free address, and the page that was
/// just brought in is returned to the requesting kernel code.
///
/// When a page is freed, the inverse occurs - the page is
/// brought into virtual memory, the current (soon to be
/// previous) last-free value is written to the first few bytes,
/// and the last-free pointer is updated to point to the
/// newly-freed page. This creates a FILO stack of freed pages
/// with no more bookkeeping necessary other than the last-free
/// physical frame pointer.
pub struct FiloPageFrameAllocator<M>
where
	M: FiloPageFrameManager,
{
	/// The manager responsible for bringing in and out
	/// physical pages from virtual memory.
	manager:   M,
	/// The last-free page frame address.
	last_free: u64,
}

impl<M> FiloPageFrameAllocator<M>
where
	M: FiloPageFrameManager,
{
	/// Creates a new FILO page frame allocator.
	#[inline]
	pub fn new(manager: M) -> Self {
		Self {
			manager,
			last_free: u64::MAX,
		}
	}

	/// Creates a new FILO page frame allocator with the given
	/// last-free page frame address.
	#[inline]
	pub fn with_last_free(manager: M, last_free: u64) -> Self {
		Self { manager, last_free }
	}

	/// Returns the last-free page frame address.
	#[inline]
	pub fn last_free(&self) -> u64 {
		self.last_free
	}
}

unsafe impl<M> PageFrameAllocate for FiloPageFrameAllocator<M>
where
	M: FiloPageFrameManager,
{
	#[allow(clippy::cast_possible_truncation)]
	fn allocate(&mut self) -> Option<u64> {
		if self.last_free == u64::MAX {
			// We're out of memory
			None
		} else {
			// Bring in the last-free page frame.
			let page_frame = self.last_free;
			self.last_free = unsafe { self.manager.read_u64(page_frame) };
			Some(page_frame)
		}
	}
}

unsafe impl<M> PageFrameFree for FiloPageFrameAllocator<M>
where
	M: FiloPageFrameManager,
{
	#[inline]
	unsafe fn free(&mut self, frame: u64) {
		assert_eq!(frame % 4096, 0, "frame is not page-aligned");

		self.manager.write_u64(frame, self.last_free);
		self.last_free = frame;
	}
}

/// A page frame manager is responsible for managing the virtual
/// memory mapping of physical pages as needed by the
/// [`FiloPageFrameAllocator`]. It is responsible for bringing
/// physical pages into virtual memory (usually at a known, fixed
/// address, given that only one page will ever need to be brought
/// in at a time; or by applying a fixed offset), and for
/// reading/writing values to the first few bytes of the page to
/// indicate the next/previous last-free page frame address as
/// needed by the allocator.
///
/// By default, all [`PhysicalAddressTranslator`]s implement this
/// trait, as they are capable of translating physical addresses
/// to virtual addresses.
///
/// # Safety
/// Implementors of this trait must ensure that the virtual memory
/// address used to bring in physical pages is safe to use and will
/// not cause any undefined behavior when read from or written to,
/// and that all memory accesses are safe and valid.
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

unsafe impl<T> FiloPageFrameManager for T
where
	T: PhysicalAddressTranslator,
{
	#[inline]
	unsafe fn read_u64(&mut self, page_frame: u64) -> u64 {
		let page_frame = self.to_virtual_addr(page_frame) as *mut u64;
		*page_frame
	}

	#[inline]
	unsafe fn write_u64(&mut self, page_frame: u64, value: u64) {
		let page_frame = self.to_virtual_addr(page_frame) as *mut u64;
		*page_frame = value;
	}
}
