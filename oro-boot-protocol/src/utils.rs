//! Rust-based utilities for populating the Oro boot protocol requests.
//!
//! This module is **optional** and is enabled via the `utils` feature.
//! See the crate documentation for information on how to populate
//! the kernel requests without using this module.
use crate::{RequestHeader, RequestTag};
use oro_common_assertions as assert;

/// A scanner for scanning for the kernel's requests.
///
/// To use, map the kernel into memory as normal, and then
/// pass the base address and length of the kernel's requests
/// segment to create a scanner.
///
/// **IMPORTANT**: You must pass a pointer to the read/write
/// area of the requests section _as it will exist in the
/// kernel's memory_, not the original memory location (if
/// you are copying the kernel to a new location).
pub struct RequestScanner {
	/// The base address of the requests segment.
	base: *mut u64,
	/// The length of the requests segment.
	len:  usize,
}

impl RequestScanner {
	/// Creates a new request scanner.
	///
	/// # Safety
	/// The caller must ensure that the `base` pointer is valid
	/// for the entire length of the requests segment.
	#[must_use]
	pub unsafe fn new(base: *mut u8, len: usize) -> Self {
		// Make sure it's aligned.
		assert::aligns_within::<u64, RequestHeader>();
		let align_offset = base.align_offset(::core::mem::align_of::<RequestHeader>());
		let len = len.saturating_sub(align_offset);
		// SAFETY(qix-): We've already aligned the pointer.
		#[allow(clippy::cast_ptr_alignment)]
		let base = base.add(align_offset).cast::<u64>();

		Self { base, len }
	}

	/// Scans for a request with the given tag.
	///
	/// If the request is found, a mutable reference to the
	/// request header is returned. If the request is not found,
	/// `None` is returned.
	///
	/// # Safety
	/// Caller must ensure no other threads are modifying the
	/// requests segment while the reference is held.
	///
	/// Furthermore, the caller must NOT call this function
	/// multiple times for the same request type, as long as
	/// reference turned by the previous call (with the same
	/// type) is still alive.
	///
	/// The caller must also ensure that the requests segment
	/// has properly been initialized; that is, any BSS
	/// (non-copied) data from the kernel ELF, has been zeroed out.
	///
	/// To better enforce this, we enforce that the returned reference
	/// not outlive this scanner object.
	#[must_use]
	pub unsafe fn get<T: RequestTag>(&self) -> Option<&mut T> {
		let mut ptr = self.base;

		// A little bit of a hack to get around the division ban.
		let shift = (::core::mem::size_of_val(&T::TAG) - 1).count_ones();
		let end = self.base.add(self.len >> shift);

		// SAFETY(qix-): We are guaranteed to have valid alignment
		// SAFETY(qix-): given that we start aligned, and iterate
		// SAFETY(qix-): on 16-byte boundaries.
		#[allow(clippy::cast_ptr_alignment)]
		while ptr < end {
			let header = &*(ptr.cast::<RequestHeader>());

			if header.magic == T::TAG {
				return Some(&mut *ptr.cast());
			}

			// Gets the alignment requirements, and then divides by
			// the tag size
			ptr = ptr.add(::core::mem::align_of::<RequestHeader>() >> shift);
		}

		None
	}
}
