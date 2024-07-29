//! Implements the internals of the `#[derive(Ser2Mem)]` derive proc macro.
//!
//! This module is only intended to be used by the [`crate::boot`] protocol
//! and thus must not be exported by the crate.
//!
//! # Safety
//! Ser2Mem is very unsafe. It has a lot of restrictions, and there's no guarantee
//! that all of those restrictions are checked by the proc macros or implementations.
//!
//! **It is not advisable to use outside of its intended purpose: the boot protocol.**

pub use oro_common_proc::Ser2Mem;

/// A namespace-preserving proxy trait that hides auto-derived (proc-macro generated)
/// iterator proxy structures when generating the `Ser2Mem` implementation for
/// a Rust struct.
pub trait Proxy {
	/// The proxy type. This is initialized by the commons lib and used to write
	/// the struct to memory.
	type Proxy<'a>: ?Sized;
}

impl<T: Proxy> Proxy for &'_ T {
	type Proxy<'a> = <T as Proxy>::Proxy<'a>;
}

/// Serializes a type to memory via a [`Serializer`].
///
/// # Safety
/// **DO NOT MANUALLY IMPLEMENT THIS TRAIT.** Use the `#[derive(Ser2Mem)]` macro.
/// Doing this manually is extremely difficult and error-prone.
pub unsafe trait Serialize: Sized {
	/// The type that is returned from the serializer and written to memory.
	type Output: Sized + 'static;

	/// Serializes this type byte-wise to the given [`Serializer`].
	///
	/// # Safety
	/// The caller MUST NOT de-reference the reference returned by this function.
	/// It is **NOT VALID** for the pre-boot stage and will **IMMEDIATELY** invoke
	/// undefined behavior.
	unsafe fn serialize<S: Serializer>(self, s: &mut S) -> Result<Self::Output, S::Error>;
}

/// A dynamic dispatch iterator wrapper.
#[allow(dead_code)]
pub struct DynIter<'a, T: Serialize>(&'a mut dyn Iterator<Item = T>);

impl<'a, T: Serialize> Iterator for DynIter<'a, T> {
	type Item = T;

	#[inline]
	fn next(&mut self) -> Option<Self::Item> {
		self.0.next()
	}
}

unsafe impl<'a, T: Serialize> Serialize for DynIter<'a, T> {
	type Output = &'static [T::Output];

	#[inline]
	unsafe fn serialize<S: Serializer>(self, s: &mut S) -> Result<Self::Output, S::Error> {
		let layout = core::alloc::Layout::new::<T::Output>();
		let base = s.align_to(layout.align())?;

		let mut count = 0;
		for item in self.0 {
			// SAFETY(qix-): This is safe because 1) we're guaranteed to be #[repr(C)], which
			// SAFETY(qix-): 2) guarantees the size is a multiple of the alignment.
			item.serialize(s)?;
			count += 1;
		}

		Ok(core::slice::from_raw_parts(base.cast(), count))
	}
}

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_primitives {
	($($T:ty),*) => {
		$(
			unsafe impl Serialize for $T {
				type Output = Self;

				#[inline]
				unsafe fn serialize<S: Serializer>(self, _s: &mut S) -> Result<Self::Output, S::Error> {
					Ok(self)
				}
			}

			impl Proxy for $T {
				type Proxy<'iter> = Self;
			}
		)*
	}
}

impl_primitives![u8, u16, u32, u64, usize, i8, i16, i32, i64, isize];

/// Writes values, linearly, to memory (similar to a cursor). Supports alignment.
///
/// # Safety
/// Implementations of this trait must ensure that the written memory is available
/// to the kernel via a simple virtual memory mapping and pointer cast.
///
/// All writes via `write` must adhere to the alignment and padding requirements
/// of the type. While alignment is not necessary to be enforced in the pre-boot
/// stage, it must be correct when read from the kernel.

pub unsafe trait Serializer {
	/// The type of error returned by the serializer.
	type Error: Sized + core::fmt::Debug + 'static;

	/// Writes the given bytes to the stream.
	///
	/// **This method purposefully does not return the target address of the written bytes.**
	/// This is to help make sure it's useless without first calling [`Self::align_to()`].
	///
	/// # Safety
	/// Must ONLY be called by auto-derived implementations of `Serialize`.
	///
	/// Callers must ensure that the bytes are properly aligned, padded,
	/// and otherwise representable data that can be cast to Rust types
	/// in the kernel.
	unsafe fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;

	/// Aligns the stream to the given alignment.
	///
	/// This method returns the **target** (in-kernel) virtual address of
	/// the first byte **after** alignment.
	///
	/// # Safety
	/// Must ONLY be called by auto-derived implementations of `Serialize`.
	///
	/// Callers must ensure that the alignment is correct for the type
	/// being written.
	unsafe fn align_to(&mut self, align: usize) -> Result<*const (), Self::Error>;

	/// Writes, byte-wise, the given type to the stream.
	///
	/// # Safety
	/// Must ONLY be called by auto-derived implementations of `Serialize`.
	unsafe fn write<T: Proxy>(&mut self, value: T) -> Result<(), Self::Error> {
		self.write_bytes(core::slice::from_raw_parts(
			core::ptr::from_ref(&value).cast::<u8>(),
			core::mem::size_of::<T>(),
		))
	}
}
