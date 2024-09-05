//! Memory address translation utilities for the pre-boot stage.

#![allow(clippy::inline_always)]

/// Translates a page frame to a virtual address, used in the pre-boot stage
/// to write kernel configuration structures.
///
/// # Safety
/// Implementors must be aware that physical addresses
/// **may not** be page aligned.
pub unsafe trait Translator: Clone + Sized + 'static {
	/// Translates a physical frame address to a virtual address
	/// returning an immutable pointer to the data.
	///
	/// # Panics
	/// Panics if the given physical address cannot fit into the
	/// target pointer type.
	#[must_use]
	fn translate<T>(&self, physical_addr: impl TryInto<usize>) -> *const T;

	/// Translates a physical frame address to a virtual address
	/// returning a mutable pointer to the data.
	///
	/// # Panics
	/// Panics if the given physical address cannot fit into the
	/// target pointer type.
	#[must_use]
	fn translate_mut<T>(&self, physical_addr: impl TryInto<usize>) -> *mut T;
}

/// An offset-based [`Translator`] that applies an offset
/// to physical frames resulting in a valid virtual address. Used in cases
/// where all memory regions have been direct-mapped.
#[derive(Clone)]
pub struct OffsetTranslator {
	/// The offset to apply to physical addresses
	/// in order to get a valid virtual address.
	///
	/// # Safety
	/// No checks are performed to ensure that the
	/// offset is correct. Further, no additional
	/// modification to the resulting address is made
	/// (e.g. sign extension, etc).
	offset: usize,
}

impl OffsetTranslator {
	/// Creates a new offset physical frame translator.
	#[must_use]
	pub fn new(offset: usize) -> Self {
		Self { offset }
	}

	/// Returns the offset applied to physical addresses.
	#[must_use]
	pub const fn offset(&self) -> usize {
		self.offset
	}
}

unsafe impl Translator for OffsetTranslator {
	#[inline(always)]
	fn translate<T>(&self, physical_addr: impl TryInto<usize>) -> *const T {
		unsafe {
			let Ok(paddr) = physical_addr.try_into() else {
				panic!("physical address is too large to fit into a pointer")
			};
			(self.offset as *const u8).add(paddr).cast()
		}
	}

	#[inline(always)]
	fn translate_mut<T>(&self, physical_addr: impl TryInto<usize>) -> *mut T {
		unsafe {
			let Ok(paddr) = physical_addr.try_into() else {
				panic!("physical address is too large to fit into a pointer")
			};
			(self.offset as *mut u8).add(paddr).cast()
		}
	}
}
