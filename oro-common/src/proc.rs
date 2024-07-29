//! Provides re-exports and supporting types for all proc-macros
//! used by the Oro kernel.

pub use oro_common_proc::{gdb_autoload_inline, paste, AsU32, AsU64, EnumIterator};

/// Allows the unit variants of an enum to be iterated over.
///
/// This trait is derived via `#[derive(EnumIterator)]`.
/// See [`EnumIterator`] for more information.
pub trait EnumIterator: Copy + Sized {
	/// Returns an iterator over all unit variants of the enum.
	fn iter_all() -> impl Iterator<Item = Self> + Sized + 'static;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn paste_noop() {
		paste! {
			let x = 18432;
		}

		assert_eq!(18432, x);
	}

	#[test]
	fn paste_single() {
		let const_a = 1294382;
		let const_b = 9238471;

		macro_rules! get_const_value {
			($ident:ident, $select:ident) => {
				paste! {
					let $ident = const_ %% $select;
				}
			};
		}

		get_const_value!(value_a, a);
		get_const_value!(value_b, b);

		assert_eq!(1294382, value_a);
		assert_eq!(9238471, value_b);
	}

	#[test]
	fn paste_multi() {
		let const_a_1 = 98382;
		let const_a_2 = 1234;
		let const_b_1 = 991833;
		let const_b_2 = 374498;

		macro_rules! get_const_value {
			($ident:ident, $select:ident, $num:tt) => {
				paste! {
					let $ident = const_ %% $select %% _ %% $num;
				}
			};
		}

		get_const_value!(value_a_1, a, 1);
		get_const_value!(value_a_2, a, 2);
		get_const_value!(value_b_1, b, 1);
		get_const_value!(value_b_2, b, 2);

		assert_eq!(98382, value_a_1);
		assert_eq!(1234, value_a_2);
		assert_eq!(991833, value_b_1);
		assert_eq!(374498, value_b_2);
	}
}
