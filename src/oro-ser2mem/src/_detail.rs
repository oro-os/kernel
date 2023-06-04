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
/// Do not implement yourself. Instead, use the `#[derive(Ser2Mem)]`
/// attribute.
pub unsafe trait Proxied {
	/// The type to be implemented.
	type Proxy;
}

/// A T=>U type relationship for serializing a Bootloader value to a
/// memory-correct Kernel memory value. For [Pod]-marked types, this
/// is a simple byte-wise copy.
///
/// To get around blanket + specialized trait implementation mixing,
/// a single trait type argument is specified, but is entirely unused.
///
/// # Safety
///
/// Do not implement yourself. If it's not implemented by default here,
/// it means that it cannot be safely used in boot structures; do not
/// try to force it. You'll most likely incur undefined behavior.
///
/// A gentle reminder that ser2mem is _not_ a general-purpose
/// serialization framework.
pub unsafe trait Serializable {
	type Target: Sized;

	/// # Safety
	/// Do not call yourself. Use the `.serialize()` method on `Serialize` types.
	///
	/// A gentle reminder that ser2mem is _not_ a general-purpose
	/// serialization framework.
	unsafe fn serialize_to<A>(self, to: *mut Self::Target, alloc: &mut A)
	where
		A: Allocator;
}

macro_rules! pod_types {
	($($t:ty),*) => {
		$(unsafe impl Serializable for $t {
			type Target = Self;

			#[inline(always)]
			unsafe fn serialize_to<A>(self, to: *mut Self, _alloc: &mut A)
			where
				A: Allocator,
			{
				*to = self;
			}
		})*
	}
}

pod_types![u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64];

/// Serialize (with ser2mem) an iterator to a new memory region and return the slice
///
/// # Safety
///
/// DO NOT CALL. This is for use by auto-generated implementations by the ser2mem
/// procedural macros.
///
/// A gentle reminder that ser2mem is _not_ a general-purpose
/// serialization framework.
pub unsafe fn serialize_iterator_to_slice<I, T, A>(iter: I, alloc: &mut A) -> &'static [T::Target]
where
	T: Serializable,
	I: Iterator<Item = T> + Clone,
	A: Allocator,
{
	let count = iter.clone().count();
	let layout = ::core::alloc::Layout::array::<T::Target>(count).unwrap();

	alloc.align(layout.align() as u64);
	let base = ::core::slice::from_raw_parts_mut(alloc.position() as *mut T::Target, count);
	alloc.allocate(layout.size() as u64);

	for (i, item) in iter.enumerate() {
		item.serialize_to(&mut base[i] as *mut T::Target, alloc);
	}

	base
}
