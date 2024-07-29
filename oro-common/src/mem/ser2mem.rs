//! Implementation of the Ser2Mem serializer using a PFA
//! and offset mapper.

use super::AddressSegment;
use crate::{
	mem::{AddressSpace, MapError, PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator},
	ser2mem::Serializer,
};

/// A [`crate::ser2mem::Serializer`] implementation that serializes
/// memory using a PFA and kernel mapper.
pub struct PfaSerializer<'a, Alloc, P, Addr>
where
	Addr: AddressSpace,
	Alloc: PageFrameAllocate + PageFrameFree,
	P: PhysicalAddressTranslator,
{
	/// The page frame allocator to use for memory allocation.
	allocator:           &'a mut Alloc,
	/// The physical address translator to use for memory translation.
	translator:          P,
	/// The address space of the kernel to use for mapping in serialized pages.
	kernel_space:        &'a Addr::SupervisorHandle,
	/// A mutable slice of the current page's bytes. Updated as pages are allocated
	/// and bytes are reserved.
	current_page_slice:  &'static [u8],
	/// The current target virtual address of the next write.
	/// SAFETY(qix-): Do not dereference this virtual address; it will invoke undefined behavior immediately.
	current_target_virt: usize,
}

impl<'a, Alloc, P, Addr> PfaSerializer<'a, Alloc, P, Addr>
where
	Addr: AddressSpace,
	Alloc: PageFrameAllocate + PageFrameFree,
	P: PhysicalAddressTranslator,
{
	/// Creates a new serializer with the given allocator and translator.
	///
	/// # Safety
	/// The offset map of the physical pages MUST NOT change at all during
	/// the lifetime of this object.
	///
	/// Further, no other active references (including from other cores)
	/// may refer to any of the memory within [`Addr::boot_info()`] during
	/// the lifetime of this object.
	pub unsafe fn new(
		allocator: &'a mut Alloc,
		translator: P,
		kernel_space: &'a Addr::SupervisorHandle,
	) -> Self {
		let (boot_info_start, _) = Addr::boot_info().range();

		Self {
			allocator,
			translator,
			kernel_space,
			current_page_slice: &mut [],
			current_target_virt: boot_info_start,
		}
	}

	/// Reserves `n` bytes of memory for writing, returning
	/// a writable slice of memory.
	///
	/// Note that the returned slice may not be the full `n` bytes
	/// requested, as it may be split across multiple pages. The caller
	/// must call this again for any remaining bytes.
	///
	/// The memory will have been properly mapped into the kernel's
	/// address space; the caller only needs to concern themselves with
	/// writing to the returned slice.
	///
	/// # Safety
	/// The returned slice's contents are undefined and must be initialized
	/// before being read from.
	///
	/// The caller **MUST** use all `n` bytes of the returned slice, and must
	/// not request more than necessary.
	// TODO(qix-): Once the `maybe_uninit_write_slice` and/or `maybe_uninit_as_bytes`
	// TODO(qix-): features are stabilized, use `MaybeUninit` here just for extra safety.
	unsafe fn reserve(&mut self, n: usize) -> Result<&'static mut [u8], MapError> {
		if n == 0 {
			return Ok(&mut []);
		}

		if self.current_page_slice.len() == 0 {
			let page_phys = self.allocator.allocate().ok_or(MapError::OutOfMemory)?;
			let current_page_virt = self.translator.to_virtual_addr(page_phys);
			Addr::boot_info().map(
				self.kernel_space,
				self.allocator,
				&self.translator,
				self.current_target_virt,
				page_phys,
			)?;

			// TODO(qix-): when we support larger page allocation sizes, this will need to be changed.
			self.current_page_slice =
				core::slice::from_raw_parts(current_page_virt as *mut u8, 4096);
		}

		debug_assert!(self.current_page_slice.len() > 0);
		let byte_count = n.min(self.current_page_slice.len());

		// SAFETY(qix-): We have to work around the borrow checker here, as we need to
		// SAFETY(qix-): return a mutable slice of the current page's bytes. We know that
		// SAFETY(qix-): the current page slice is valid and has not been deallocated,
		// SAFETY(qix-): so we can safely cast it to a mutable slice. We also know we won't
		// SAFETY(qix-): be writing to the same memory from multiple threads, so this is safe.
		let slice = core::slice::from_raw_parts_mut(
			self.current_page_slice.as_ptr() as *mut u8,
			byte_count,
		);

		self.current_page_slice = &self.current_page_slice[byte_count..];

		Ok(slice)
	}
}

unsafe impl<'a, Alloc, P, Addr> Serializer for PfaSerializer<'a, Alloc, P, Addr>
where
	Addr: AddressSpace,
	Alloc: PageFrameAllocate + PageFrameFree,
	P: PhysicalAddressTranslator,
{
	type Error = MapError;

	unsafe fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
		let mut remaining = bytes;
		while !remaining.is_empty() {
			let slice = self.reserve(remaining.len())?;
			let (to_write, rest) = remaining.split_at(slice.len());
			slice.copy_from_slice(to_write);
			remaining = rest;
		}

		Ok(())
	}

	unsafe fn align_to(&mut self, align: usize) -> Result<*const (), Self::Error> {
		let ptr = self.current_target_virt as *const ();
		match ptr.align_offset(align) {
			0 => Ok(ptr),
			usize::MAX => Err(MapError::OutOfMemory),
			mut offset => {
				while offset > 0 {
					let slice = self.reserve(offset)?;
					slice.fill(0);
					offset -= slice.len();
				}
				Ok(self.current_target_virt as *const ())
			}
		}
	}
}
