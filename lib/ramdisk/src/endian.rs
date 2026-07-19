//! Endianness wrapper types.

/// Denotes a big endian value of type `T`.
#[derive(Copy)]
#[repr(transparent)]
pub struct BigEndian<T>(T);

impl<T: BeToNe> BigEndian<T> {
	/// Converts the big endian value to native endianness.
	pub fn to_ne(&self) -> T {
		self.0.to_ne()
	}
}

impl<T: BeToNe> From<T> for BigEndian<T> {
	fn from(value: T) -> Self {
		Self(value.to_be())
	}
}

impl<T> Clone for BigEndian<T>
where
	T: Clone,
{
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T> core::fmt::Debug for BigEndian<T>
where
	T: BeToNe + core::fmt::Debug,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		self.to_ne().fmt(f)
	}
}

/// Private mod for sealed types.
mod private {
	/// Marker trait for internal types.
	pub trait Sealed {}
}

/// Converts the numeric type to native endianness.
pub trait BeToNe: private::Sealed {
	/// Converts the numeric type to native endianness.
	fn to_ne(&self) -> Self;
	/// Converts the numeric type to big endian.
	fn to_be(&self) -> Self;
}

macro_rules! impl_be_to_ne {
	($($t:ty),*) => {
		$(
			impl private::Sealed for $t {}
			impl BeToNe for $t {
				fn to_ne(&self) -> Self {
					Self::from_be(*self)
				}

				fn to_be(&self) -> Self {
					Self::to_be(*self)
				}
			}
		)*
	};
}

impl_be_to_ne!(u16, u32, u64, i16, i32, i64);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn from_ne() {
		let be = BigEndian::from(1234u32);
		let raw = be.0;
		assert_eq!(raw, (1234u32).to_be());
	}

	#[test]
	fn from_be_to_ne() {
		let be = BigEndian(0xAABBCCDDu32);
		let ne = be.to_ne();
		#[cfg(target_endian = "little")]
		{
			assert_eq!(ne, 0xDDCCBBAA);
		}
		#[cfg(target_endian = "big")]
		{
			assert_eq!(ne, 0xAABBCCDD);
		}
	}
}
