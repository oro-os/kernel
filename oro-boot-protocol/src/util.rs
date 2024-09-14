//! Rust-based utilities for populating the Oro boot protocol requests.
//!
//! This module is **optional** and is enabled via the `utils` feature.
//! See the crate documentation for information on how to populate
//! the kernel requests without using this module.
use crate::{Request, RequestHeader, RequestTag, Tag};
use oro_macro::assert;

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
	base: *mut Tag,
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
		assert::aligns_within::<Tag, RequestHeader>();
		let align_offset = base.align_offset(::core::mem::align_of::<RequestHeader>());
		let len = len.saturating_sub(align_offset);
		// SAFETY(qix-): We've already aligned the pointer.
		#[expect(clippy::cast_ptr_alignment)]
		let base = base.add(align_offset).cast::<Tag>();

		Self { base, len }
	}

	/// Scans for a request with the given tag.
	///
	/// If the request is found, a mutable reference to the
	/// request header is returned. If the request is not found,
	/// `None` is returned.
	///
	/// > **Note**: This function is tricky to make safe
	/// > and is inefficient to use more than once.
	/// > It is recommended to use the `iter_mut()` function
	/// > instead with a `match` statement.
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
		let shift = (::core::mem::size_of::<Tag>() - 1).count_ones();
		let end = self.base.add(self.len >> shift);

		// SAFETY(qix-): We are guaranteed to have valid alignment
		// SAFETY(qix-): given that we start aligned, and iterate
		// SAFETY(qix-): on 16-byte boundaries.
		#[expect(clippy::cast_ptr_alignment)]
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

	/// Attempts to write response data to the Kernel.
	///
	/// Returns an error if either the request is not found (i.e.
	/// not requested by the kernel) or if the revision of the
	/// request does not match the revision of the response.
	pub fn try_send<R: crate::DataRevision>(&self, data: R) -> Result<(), TrySendError>
	where
		R::Request: RequestData,
	{
		// SAFETY(qix-): We're controlling the lifetimes and the
		// SAFETY(qix-): references, so none of the safety invariants
		// SAFETY(qix-): specified by the scanner are violated.
		let Some(req) = (unsafe { self.get::<<R as crate::Data>::Request>() }) else {
			return Err(TrySendError::NotRequested);
		};

		if req.revision() != R::REVISION {
			return Err(TrySendError::WrongRevision {
				expected: req.revision(),
			});
		}

		// SAFETY(qix-): We've already checked the revision and tag, and the unions
		// SAFETY(qix-): are marked as `#[repr(C)]` so we know that a write to the
		// SAFETY(qix-): union's base address is safe (if it was not `#[repr(C)]`,
		// SAFETY(qix-): the union may have non-zero field offsets).
		unsafe {
			req.response_data().cast::<R>().write(data);
		}

		req.mark_populated();

		Ok(())
	}

	/// Returns an iterator over all requests in the segment.
	///
	/// # Safety
	/// The `response` field of the returned request header **must not**
	/// be used. It is only safe to use the `Request` element of the returned
	/// tuple.
	#[must_use]
	pub unsafe fn iter_mut(&self) -> RequestScannerIter {
		// A little bit of a hack to get around the division ban.
		let shift = (::core::mem::size_of::<Tag>() - 1).count_ones();
		// SAFETY(qix-): Len will never have the high bit set.
		let end = unsafe { self.base.add(self.len >> shift) };

		RequestScannerIter {
			ptr: self.base,
			end,
			_phantom: ::core::marker::PhantomData,
		}
	}
}

/// An error that can occur when attempting to send a response.
#[derive(Debug, Clone, Copy)]
pub enum TrySendError {
	/// The request was not requested by the kernel.
	NotRequested,
	/// The request was requested, but the revision was incorrect.
	WrongRevision {
		/// The revision that the kernel instead requested
		expected: u64,
	},
}

/// An iterator over the requests in a request segment.
pub struct RequestScannerIter<'a> {
	/// The next pointer we'll attempt to read.
	ptr:      *mut Tag,
	/// The first pointer after the end of the segment.
	end:      *mut Tag,
	/// Just enforces that the lifetime is used, keeping
	/// this iterator from being used after the scanner
	/// that created it drops.
	_phantom: ::core::marker::PhantomData<&'a ()>,
}

impl<'a> Iterator for RequestScannerIter<'a> {
	type Item = (&'a mut RequestHeader, Request<'a>);

	fn next(&mut self) -> Option<Self::Item> {
		while self.ptr < self.end {
			let maybe_req = unsafe { super::request_from_tag(&mut *(self.ptr.cast())) };
			self.ptr = unsafe { self.ptr.add(1) };
			if maybe_req.is_some() {
				return maybe_req;
			}
		}

		None
	}
}

/// Lower level data manipulation for a request.
///
/// Used internally by the request scanner;
/// you should probably use higher level methods
/// or direct field accesses instead, as this trait
/// is only enabled with the `utils` feature.
pub trait RequestData: crate::RequestTag {
	/// Returns a mutable pointer to the base of the response data.
	///
	/// Used internally by the request scanner; you should probably
	/// use the higher level `response()` method instead.
	///
	/// # Safety
	/// The caller must ensure that writes to this pointer
	/// are valid data responses for the request, and that
	/// the written response revision matches the request revision.
	unsafe fn response_data(&mut self) -> *mut u8;

	/// Returns the revision of the request.
	///
	/// Used internally by the request scanner; you should
	/// probably use the `revision` field directly.
	fn revision(&self) -> u64;

	/// Marks the request as populated.
	fn mark_populated(&mut self);
}

/// Mutator utility trait for boot protocol types
/// that chain to the next item in the list.
#[expect(private_bounds)]
pub trait SetNext: crate::macros::Sealed + 'static {
	/// Sets the next pointer to the given physical address.
	fn set_next(&mut self, next: u64);
}
