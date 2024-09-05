//! Provides macros for a quick way of creating register fields.

/// A quick way of creating register fields.
#[macro_export]
macro_rules! field {
	($base_ty:ty, $name:ident, $start_bit:literal, $end_bit:literal, $T:ty, $doc:literal) => {
		::oro_macro::paste! {
			#[doc = concat!("Gets the `", stringify!($name), "` field.\n\n", $doc)]
			#[allow(dead_code, clippy::identity_op)]
			#[must_use]
			pub fn $name(self) -> $T {
				<$T>::from((self.0 >> $start_bit) & ((1 << ($end_bit - $start_bit + 1)) - 1))
			}

			#[doc = concat!("Sets the `", stringify!($name), "` field.\n\n", $doc)]
			#[allow(dead_code, clippy::identity_op)]
			pub fn set_ %% $name(&mut self, val: $T) {
				let val: $base_ty = val.into();
				let val = val & ((1 << ($end_bit - $start_bit + 1)) - 1);
				self.0 &= !(((1 << ($end_bit - $start_bit + 1)) - 1) << $start_bit);
				self.0 |= val << $start_bit;
			}
		}
	};

	($base_ty:ty, $name:ident, $bit:literal, $T:ty, $doc:literal) => {
		::oro_macro::paste! {
			#[doc = concat!("Gets the `", stringify!($name), "` field.\n\n", $doc)]
			#[allow(dead_code, clippy::identity_op)]
			#[must_use]
			pub fn $name(self) -> $T {
				<$T>::from(((self.0 >> $bit) & 1) == 1)
			}

			#[doc = concat!("Sets the `", stringify!($name), "` field.\n\n", $doc)]
			#[allow(dead_code, clippy::identity_op)]
			pub fn set_ %% $name(&mut self, val: $T) {
				let val: $base_ty = val.into();
				self.0 &= !(1 << $bit);
				self.0 |= val << $bit;
			}
		}
	};

	($name:ident, $start_bit:literal, $end_bit:literal, $T:ty, $doc:literal) => {
		$crate::reg::field::field!(u64, $name, $start_bit, $end_bit, $T, $doc);
	};

	($name:ident, $bit:literal, $T:ty, $doc:literal) => {
		$crate::reg::field::field!(u64, $name, $bit, $T, $doc);
	};

	($name:ident, $start_bit:literal, $end_bit:literal, $doc:literal) => {
		$crate::reg::field::field!(u64, $name, $start_bit, $end_bit, u64, $doc);
	};

	($name:ident, $bit:literal, $doc:literal) => {
		$crate::reg::field::field!(u64, $name, $bit, bool, $doc);
	};
}

pub use field;

/// A wrapper around the [`field!`] macro that works with a `u32` base type.
#[macro_export]
macro_rules! field32 {
	($name:ident, $start_bit:literal, $end_bit:literal, $T:ty, $doc:literal) => {
		$crate::reg::field::field!(u32, $name, $start_bit, $end_bit, $T, $doc);
	};

	($name:ident, $bit:literal, $T:ty, $doc:literal) => {
		$crate::reg::field::field!(u32, $name, $bit, $T, $doc);
	};

	($name:ident, $start_bit:literal, $end_bit:literal, $doc:literal) => {
		$crate::reg::field::field!(u32, $name, $start_bit, $end_bit, u32, $doc);
	};

	($name:ident, $bit:literal, $doc:literal) => {
		$crate::reg::field::field!(u32, $name, $bit, bool, $doc);
	};
}

pub use field32;
