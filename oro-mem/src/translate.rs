//! Memory address translation utilities for the pre-boot stage.
#![expect(clippy::inline_always)]

/// Translates a page frame to a virtual address, used in the pre-boot stage
/// to write kernel configuration structures.
///
/// # Safety
/// Implementors must be aware that physical addresses
/// **may not** be page aligned.
pub unsafe trait Translate {
	/// Translates a physical frame address to a virtual address.
	///
	/// # Panics
	/// Panics if the given physical address cannot fit into a `usize`.
	#[must_use]
	fn translate(&self, physical_addr: u64) -> usize;
}

/// An offset-based [`Translate`] that applies an offset
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
	pub const fn new(offset: usize) -> Self {
		Self { offset }
	}

	/// Returns the offset applied to physical addresses.
	#[must_use]
	pub const fn offset(&self) -> usize {
		self.offset
	}

	/// Sets the offset applied to physical addresses.
	///
	/// # Safety
	/// Caller must ensure that the offset is valid and
	/// assumes responsibility for all side effects of changing
	/// the offset at runtime.
	pub unsafe fn set_offset(&mut self, offset: usize) {
		self.offset = offset;
	}
}

unsafe impl Translate for OffsetTranslator {
	#[inline(always)]
	fn translate(&self, physical_addr: u64) -> usize {
		::oro_macro::assert::fits_within::<usize, u64>();
		(self.offset as u64 + physical_addr).try_into().unwrap()
	}
}
