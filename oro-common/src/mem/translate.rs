//! Memory address translation utilities for the pre-boot stage.

#![allow(clippy::inline_always)]

/// Translates a page frame to a virtual address, used in the pre-boot stage
/// to write kernel configuration structures.
///
/// # Safety
/// Implementors must be aware that physical addresses
/// **may not** be page aligned.
pub unsafe trait PhysicalAddressTranslator: Clone + Sized + 'static {
	/// Translates a physical frame address to a virtual address.
	#[must_use]
	fn to_virtual_addr(&self, physical_addr: u64) -> usize;
}

/// An offset-based [`PhysicalAddressTranslator`] that applies an offset
/// to physical frames resulting in a valid virtual address. Used in cases
/// where all memory regions have been direct-mapped.
#[derive(Clone)]
pub struct OffsetPhysicalAddressTranslator {
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

impl OffsetPhysicalAddressTranslator {
	/// Creates a new offset physical frame translator.
	///
	/// # Safety
	/// Caller must ensure the offset is correct and that all
	/// memory is direct mapped.
	///
	/// Further, **caller must ensure that a cast between
	/// `u64` and `usize` is valid.**
	#[must_use]
	pub unsafe fn new(offset: usize) -> Self {
		Self { offset }
	}
}

unsafe impl PhysicalAddressTranslator for OffsetPhysicalAddressTranslator {
	#[allow(clippy::cast_possible_truncation)]
	#[inline(always)]
	fn to_virtual_addr(&self, physical_addr: u64) -> usize {
		physical_addr as usize + self.offset
	}
}
