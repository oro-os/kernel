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
