//! **Do not use the items in this module directly** These are
//! supplemental items for use by the procedural macros and are
//! NOT to be used directly!
//!
//! Since these types are used by procedural macros, the traits/markers
//! cannot be sealed. **This does not mean they are free to be used.**

use crate::Allocator;

/// Marker trait for POD (plain ol' data) datatypes that are safe
/// to copy around without any special handling. Note that not all
/// types that would be
///
/// # Safety
/// DO NOT implement yourself. Use the `#[derive(Pod)]` attribute
/// from this crate.
pub unsafe trait Pod: 'static + Copy {}

/// # Safety
///
/// Does some erotic dancing with pointers. Do not implement yourself;
/// the only method to implement this trait is to use `#[derive(Ser2Mem)]`.
///
/// To get a reference to the proxy type, use `Proxy![OriginalType]`.
pub unsafe trait Proxy {
	/// # Safety
	///
	/// Among other things (such as writing invalid slices to memory),
	/// `alloc.position()` must report an address that is aligned to
	/// `Self`'s alignment requirements prior to calling this function.
	/// This function will only align child elements prior to serializing
	/// them, but expects the caller to have done so beforehand.
	unsafe fn serialize<A>(self, alloc: &mut A)
	where
		A: Allocator;
}

/// # Safety
///
/// Do not implement yourself. Instead, use the `#[derive(Ser2Mem)]`
/// attribute.
pub unsafe trait Proxied {
	/// The type to be implemented.
	type Proxy: Proxy;
}

/// A T=>U type relationship for serializing a Bootloader value to a
/// memory-correct Kernel memory value. For [Pod]-marked types, this
/// is a simple byte-wise copy.
///
/// # Safety
///
/// Do not implement yourself. If it's not implemented by default here,
/// it means that it cannot be safely used in boot structures; do not
/// try to force it. You'll most likely incur undefined behavior.
///
/// A gentle reminder that ser2mem is _not_ a general-purpose
/// serialization framework.
pub unsafe trait Serializable<T: Sized> {
	/// # Safety
	/// Do not call yourself. Use the `serialize!()` macro.
	///
	/// A gentle reminder that ser2mem is _not_ a general-purpose
	/// serialization framework.
	unsafe fn serialize_to<A>(&self, to: *mut T, alloc: &mut A)
	where
		A: Allocator;
}

// For any copyable (trivial) type, simply copy them.
unsafe impl<T> Serializable<T> for T
where
	T: Pod,
{
	#[inline(always)]
	unsafe fn serialize_to<A>(&self, to: *mut T, _alloc: &mut A)
	where
		A: Allocator,
	{
		*to = *self;
	}
}

macro_rules! pod_types {
	($($t:ty),*) => {
		$(unsafe impl Pod for $t {})*
	}
}

pod_types![u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, bool];
