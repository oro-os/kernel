//! Provides the [`Proxy`] type, which is a type-safe, type-erased container for a single value
//! within a fixed size buffer.

use super::assertions::{AssertFits, AssertNoDrop};

/// An opaque wrapper around an initialized value.
///
/// # Safety
/// No attempt to infer the contents of this structure
/// should be made whatsoever. Use the functions provided
/// to you by [`Proxy`] to interact with the value.
/// Do not copy or move this value around.
pub struct ProxyValue<const SIZE: usize> {
	/// The type ID of the value. Used to ensure
	/// that subsequent accesses are of the correct type.
	tid: core::any::TypeId,
	/// The type-erased buffer containing the value.
	/// SAFETY: This buffer is guaranteed to be at least the size of the value.
	/// SAFETY: However, it's important to note that any bytes beyond the stored type's
	/// SAFETY: size are uninitialized and should not be accessed.
	buf: [u8; SIZE],
}

/// A type-safe, type-erased container for a single value within a fixed size buffer.
///
/// This type is useful for storing a single value of an unknown type within a fixed-size buffer,
/// namely where the type is known at compile time but comes from somewhere that cannot be used
/// as a static (e.g. from a generic parameter).
///
/// This type does its best to ensure that the value is of the correct type and size, and that
/// the types used to reference the value are correct across all calls.
pub enum Proxy<const SIZE: usize> {
	/// The value is uninitialized.
	Uninit,
	/// The value is initialized and can be referenced or taken.
	Value(ProxyValue<SIZE>),
}

impl<const SIZE: usize> Proxy<SIZE> {
	/// Create a [`Proxy::Value`] from a value.
	///
	/// The value must have a size less than or equal to `SIZE`, and
	/// cannot have a destructor (`impl Drop` or have any fields that
	/// implement `Drop`). This is enforced at compile time.
	pub fn from<T: Sized + AssertFits<SIZE> + AssertNoDrop + 'static>(v: T) -> Self {
		() = <T as AssertFits<SIZE>>::ASSERT;
		() = <T as AssertNoDrop>::ASSERT;

		unsafe {
			// SAFETY: This is technically undefined behavior, but given that
			// SAFETY: it's an array of adequate size that we'll be copying into
			// SAFETY: immediately after, this is safe to do. It is the equivalent
			// SAFETY: of a `.write()` call, but with a different type.
			// TODO(qix-): Is there a better way to do this?
			#[allow(clippy::uninit_assumed_init)]
			let mut buf = core::mem::MaybeUninit::<[u8; SIZE]>::uninit().assume_init();
			core::ptr::copy_nonoverlapping(
				#[allow(clippy::ref_as_ptr)]
				(&v as *const T).cast::<u8>(),
				buf.as_mut_ptr(),
				core::mem::size_of::<T>(),
			);
			core::mem::forget(v);
			Self::Value(ProxyValue {
				tid: core::any::TypeId::of::<T>(),
				buf,
			})
		}
	}

	/// Get a reference to the value, if it is of the correct type.
	///
	/// If the proxy is `Uninit`, or if `T` does not match the same type,
	/// from the call to `from()`, this function will return `None`.
	// NOTE(qix-): I'm using the lifetimes here as both documentation as well
	// NOTE(qix-): as a safeguard against edits such that modifications don't
	// NOTE(qix-): break any lifetime guarantees in the future.
	#[allow(clippy::needless_lifetimes)]
	pub fn as_ref<'a, T: Sized + AssertFits<SIZE> + AssertNoDrop + 'static>(
		&'a self,
	) -> Option<&'a T> {
		() = <T as AssertFits<SIZE>>::ASSERT;
		() = <T as AssertNoDrop>::ASSERT;

		match self {
			Proxy::Uninit => None,
			Proxy::Value(ProxyValue { tid, buf: _ }) if tid != &core::any::TypeId::of::<T>() => {
				None
			}
			Proxy::Value(ProxyValue { tid: _, buf }) => unsafe { Some(&*(buf.as_ptr().cast())) },
		}
	}

	/// Take back the underlying value, if it is of the correct type.
	/// Sets the proxy to `Uninit` if the value is taken.
	///
	/// If the proxy is `Uninit`, or if `T` does not match the same type,
	/// from the call to `from()`, this function will return `None`.
	pub fn take<T: Sized + AssertFits<SIZE> + AssertNoDrop + 'static>(&mut self) -> Option<T> {
		() = <T as AssertFits<SIZE>>::ASSERT;
		() = <T as AssertNoDrop>::ASSERT;

		if let Proxy::Value(ProxyValue { tid, buf: _ }) = self {
			if tid != &core::any::TypeId::of::<T>() {
				return None;
			}
		}

		match core::mem::replace(self, Proxy::Uninit) {
			Proxy::Uninit => None,
			Proxy::Value(ProxyValue { tid: _, buf }) => unsafe {
				let mut v = core::mem::MaybeUninit::<T>::uninit();
				core::ptr::copy_nonoverlapping(
					buf.as_ptr(),
					v.as_mut_ptr().cast::<u8>(),
					core::mem::size_of::<T>(),
				);
				Some(v.assume_init())
			},
		}
	}
}
