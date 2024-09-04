//! Provides a preboot page frame allocator
//! to be used to write variable length Oro kernel request
//! structures to memory.

use core::mem::MaybeUninit;
use oro_boot_protocol::{MemoryMapEntry as OroMemRe, MemoryMapEntryType as OroMemTy};

/// A simple, allocate-only page frame allocator for use in the
/// preboot stage of the Oro kernel, used to write variable-length
/// Oro kernel request structures to memory.
#[derive(Clone)]
pub struct PrebootPfa<M: Into<OroMemRe> + Clone, I: Iterator<Item = M> + Clone> {
	/// Iterator over the memory map.
	iter:           I,
	/// The linear offset to apply to physical addresses.
	linear_offset:  u64,
	/// The current physical address base
	current_base:   u64,
	/// The remaining size in the current region.
	remaining_size: u64,
	/// The number of bytes taken from the memory map.
	used:           u64,
	/// A copy of the original iterator, to be used when
	/// consuming the PFA to write the memory map for the
	/// kernel request.
	original_iter:  I,
}

impl<M: Into<OroMemRe> + Clone, I: Iterator<Item = M> + Clone> PrebootPfa<M, I> {
	/// Creates a new preboot page frame allocator from a memory map iterator.
	///
	/// The iterator must convert any preboot memory region types into
	/// Oro memory region types. See below for how to handle the `used` field.
	///
	/// The `next` field is unused and written when consuming the PFA. Set it to 0.
	///
	/// # `used` Field
	/// The `used` field on the memory region struct indicates how many bytes
	/// of an otherwise **usable** memory region are being used by the bootloader,
	/// and can be reclaimed after the Kernel has processed any boot-time information.
	///
	/// For all general purpose **immediately** usable regions of memory, set the
	/// type to [`oro_boot_protocol::MemoryMapEntryType::Usable`], and set the
	/// `used` field to 0.
	///
	/// For all bootloader reclaimable regions, set the type to `Usable` as well,
	/// but set the `used` property to the number of bytes in the region.
	///
	/// If the bootloader has used only part of a region, set the `used` field to
	/// the number of bytes used.
	///
	/// These counts **must not** affect the `length` field.
	///
	/// For all non-usable regions, this field should be set to 0 (but is otherwise
	/// ignored by the PFA and kernel).
	#[must_use]
	pub fn new(iter: I, linear_offset: u64) -> Self {
		Self {
			iter: iter.clone(),
			linear_offset,
			current_base: 0,
			remaining_size: 0,
			used: 0,
			original_iter: iter,
		}
	}

	/// Allocates a page frame of 4096 bytes (aligned to 4096 bytes).
	#[must_use]
	pub fn allocate_page(&mut self) -> Option<u64> {
		let (phys, _) = self.allocate::<AlignedPage>()?;
		Some(phys)
	}

	/// Allocates an object of the given size in bytes.
	/// Returns the physical address at which the item was allocated,
	/// as well a static mut ref to the item.
	///
	/// If there is no memory available, returns `None`.
	///
	/// # Panics
	/// Panics if the size or alignment of the object cannot fit within a `u64`.
	#[must_use]
	pub fn allocate<T: Sized>(&mut self) -> Option<(u64, &'static mut MaybeUninit<T>)> {
		loop {
			// Do we need to pull in a new region?
			while self.remaining_size < 4096 {
				let mut entry = self.iter.next()?.into();
				while entry.ty != OroMemTy::Usable {
					entry = self.iter.next()?.into();
				}

				let base = entry.base + entry.used;
				let length = entry.length - entry.used;
				if length < 4096 {
					self.used += length;
					continue;
				}

				// Align the base to a 4096-byte boundary
				let next_page = (base + 4095) & !4095;
				let offset_bytes = next_page - base;
				self.current_base = next_page;
				self.remaining_size = length - offset_bytes;
				self.used += offset_bytes;
			}

			// Align ourselves to the alignment of the type.
			let layout = core::alloc::Layout::new::<T>();
			let align = u64::try_from(layout.align()).unwrap();
			let size = u64::try_from(layout.size()).unwrap();
			let next_base = (self.current_base + align - 1) & !(align - 1);
			let align_offset = next_base - self.current_base;

			#[cfg(debug_assertions)]
			{
				let current_page = self.current_base >> 12;
				let next_page_end = (next_base + size) >> 12;
				if current_page != next_page_end {
					// Mark the new page as allocated
					oro_debug::__oro_dbgutil_pfa_alloc(next_page_end << 12);
				}
			}

			if self.remaining_size < (align_offset + size) {
				// "Use up" the rest of the region
				self.used += self.remaining_size;
				self.remaining_size = 0;
				continue;
			}

			let total_bytes = align_offset + size;
			self.current_base = next_base + size;
			self.remaining_size -= total_bytes;
			self.used += total_bytes;

			let ptr = (self.linear_offset + next_base) as *mut T;
			// SAFETY(qix-): We have done our due diligence to ensure that the
			// SAFETY(qix-): memory is available and properly aligned. Further,
			// SAFETY(qix-): as far as we're concerned here, this is the only place
			// SAFETY(qix-): a mutable reference will be created pointing to this
			// SAFETY(qix-): memory.
			let uninit = unsafe { &mut *(ptr as *mut MaybeUninit<T>) };
			return Some((next_base, uninit));
		}
	}

	/// Writes the memory map to a region in memory and returns the
	/// physical address of the first entry.
	///
	/// This consumes the PFA and guarantees that it has written the
	/// appropriate memory map types for the Oro kernel to use them,
	/// including the "bootloader reclaimable" entries that were
	/// used by this very PFA.
	///
	/// Returns `None` if there is not enough memory to write the
	/// memory map.
	#[must_use]
	pub fn write_memory_map(mut self) -> Option<u64> {
		// Get a total count of bytes, including the size of
		// all entries of the map itself.
		let mut total_bytes = {
			let mut this = self.clone();
			for _ in this.original_iter.clone() {
				let _ = this.allocate::<OroMemRe>()?;
			}
			this.used
		};

		let mut last_phys = 0;
		for mut region in self.original_iter.clone().map(Into::into) {
			let (phys, entry) = self.allocate::<OroMemRe>()?;

			if region.ty == OroMemTy::Usable {
				let unused = region.length - region.used;
				let additionally_used = unused.min(total_bytes);
				total_bytes -= additionally_used;
				region.used += additionally_used;
			}

			region.next = last_phys;

			entry.write(region);
			last_phys = phys;
		}

		Some(last_phys)
	}
}

unsafe impl<M: Into<OroMemRe> + Clone, I: Iterator<Item = M> + Clone>
	oro_common::mem::pfa::alloc::PageFrameAllocate for PrebootPfa<M, I>
{
	fn allocate(&mut self) -> Option<u64> {
		self.allocate_page()
	}
}

unsafe impl<M: Into<OroMemRe> + Clone, I: Iterator<Item = M> + Clone>
	oro_common::mem::pfa::alloc::PageFrameFree for PrebootPfa<M, I>
{
	unsafe fn free(&mut self, _frame: u64) {
		panic!("preboot PFA cannot free frames");
	}
}

#[doc(hidden)]
#[repr(align(4096))]
#[allow(dead_code)]
struct AlignedPage([u8; 4096]);
