//! Supporting types for the `#[derive(EnumIterator)]` macro.

/// Allows the unit variants of an enum to be iterated over.
///
/// This trait is derived via `#[derive(EnumIterator)]`.
/// See [`EnumIterator`] for more information.
pub trait EnumIterator: Copy + Sized {
	/// Returns an iterator over all unit variants of the enum.
	fn iter_all() -> impl Iterator<Item = Self> + Sized + 'static;
}
