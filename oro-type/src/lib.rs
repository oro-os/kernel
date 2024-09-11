//! Simple primitive types and associated traits.
//!
//! This crate consists more or less of primitive type
//! wrappers and associated traits for them (e.g.
//! forced endianness types).
#![cfg_attr(not(test), no_std)]
#![expect(clippy::inline_always, clippy::wrong_self_convention)]

use core::marker::PhantomData;

/// Wrapper around numeric types that enforces a specific
/// endianness.
///
/// Meant primarily to be used in structs whereby pointers
/// or buffers are casted to the struct type and where reads
/// of numeric fields are guaranteed to be in a certain endianness
/// regardless of host byte order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct Endian<T: Numeric, E: Endianness>(T, PhantomData<E>);

impl<T, E> core::fmt::Debug for Endian<T, E>
where
	T: Numeric + core::fmt::Debug,
	E: Endianness,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Debug::fmt(&(*self).read(), f)
	}
}

impl<T, E> core::fmt::Display for Endian<T, E>
where
	T: Numeric + core::fmt::Display,
	E: Endianness,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		core::fmt::Display::fmt(&(*self).read(), f)
	}
}

impl<T: Numeric, E: Endianness> Endian<T, E> {
	/// Creates a new [`Endian`] value with the specified endianness,
	/// taking a value of type `T` without changing its endianness.
	///
	/// **Note:** This type is more or less meant to be used as a
	/// casted-to type (e.g. a struct field's type whereby the struct
	/// is cast to from some pointer or buffer).
	#[inline(always)]
	pub const fn with_unchanged(value: T) -> Self {
		Self(value, PhantomData)
	}

	/// Reads the value.
	///
	/// The same as calling `.into()` but doesn't
	/// require type annotations.
	#[inline(always)]
	#[must_use]
	pub fn read(self) -> T {
		E::from_endian(self.0)
	}

	/// Writes a new value.
	///
	/// The same as calling `.from()` but doesn't
	/// require type annotations.
	#[inline(always)]
	pub fn write(&mut self, value: T) {
		self.0 = E::to_endian(value);
	}
}

/// Specifies the endianness when using [`Endian`].
pub trait Endianness: Clone + Copy + Default {
	/// Converts a value from the specified endianness to the host endianness.
	fn from_endian<T: Numeric>(value: T) -> T;

	/// Converts a value from the host endianness to the specified endianness.
	fn to_endian<T: Numeric>(value: T) -> T;
}

/// Little-endian endianness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LittleEndian;

impl Endianness for LittleEndian {
	#[inline(always)]
	fn from_endian<T: Numeric>(value: T) -> T {
		value.from_le()
	}

	#[inline(always)]
	fn to_endian<T: Numeric>(value: T) -> T {
		value.to_le()
	}
}

/// Big-endian endianness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct BigEndian;

impl Endianness for BigEndian {
	#[inline(always)]
	fn from_endian<T: Numeric>(value: T) -> T {
		value.from_be()
	}

	#[inline(always)]
	fn to_endian<T: Numeric>(value: T) -> T {
		value.to_be()
	}
}

/// A big endian [`Numeric`] type.
pub type Be<T> = Endian<T, BigEndian>;

/// A little endian [`Numeric`] type.
pub type Le<T> = Endian<T, LittleEndian>;

#[doc(hidden)]
mod private {
	pub trait Sealed {}
}

/// Trait for numeric primitive types.
pub trait Numeric:
	Clone + Copy + PartialEq + PartialOrd + core::fmt::Debug + core::fmt::Display + private::Sealed
{
	/// Converts the value to little-endian.
	#[must_use]
	fn to_le(self) -> Self;

	/// Converts the value to big-endian.
	#[must_use]
	fn to_be(self) -> Self;

	/// Converts the value from little-endian.
	#[must_use]
	fn from_le(self) -> Self;

	/// Converts the value from big-endian.
	#[must_use]
	fn from_be(self) -> Self;
}

/// Implements [`Numeric`] for the specified primitive types.
macro_rules! impl_numeric {
	($($ty:ty),*) => {
		$(
			impl private::Sealed for $ty {}
			impl Numeric for $ty {
				#[inline(always)]
				fn to_le(self) -> Self {
					mod p {
						#[inline(always)]
						pub fn to_le(value: $ty) -> $ty {
							value.to_le()
						}
					}

					p::to_le(self)
				}

				#[inline(always)]
				fn to_be(self) -> Self {
					mod p {
						#[inline(always)]
						pub fn to_be(value: $ty) -> $ty {
							value.to_be()
						}
					}

					p::to_be(self)
				}

				#[inline(always)]
				fn from_le(self) -> Self {
					mod p {
						#[inline(always)]
						pub fn from_le(value: $ty) -> $ty {
							<$ty>::from_le(value)
						}
					}

					p::from_le(self)
				}

				#[inline(always)]
				fn from_be(self) -> Self {
					mod p {
						#[inline(always)]
						pub fn from_be(value: $ty) -> $ty {
							<$ty>::from_be(value)
						}
					}

					p::from_be(self)
				}
			}

			impl<E: Endianness> From<Endian<$ty, E>> for $ty {
				#[inline(always)]
				fn from(value: Endian<$ty, E>) -> Self {
					E::from_endian(value.0)
				}
			}

			impl<E: Endianness> From<$ty> for Endian<$ty, E> {
				#[inline(always)]
				fn from(value: $ty) -> Self {
					Self(E::to_endian(value), PhantomData)
				}
			}
		)*
	};
}

impl_numeric!(
	u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, usize, isize
);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_endian() {
		let forward = 0x1234_5678_u32;
		let back = 0x7856_3412_u32;
		let le = Le::from(forward);
		let be = Be::from(forward);

		#[cfg(target_endian = "little")]
		{
			assert_eq!(forward, le.0);
			assert_eq!(back, be.0);
			assert_eq!(forward, le.into());
			assert_eq!(forward, be.into());
		}

		#[cfg(target_endian = "big")]
		{
			assert_eq!(back, le.0);
			assert_eq!(forward, be.0);
			assert_eq!(forward, le.into());
			assert_eq!(forward, be.into());
		}
	}
}
