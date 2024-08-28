//! Procedural macros and supporting types for the Oro kernel.
//!
//! This crate re-exports and provides the supplementary types
//! for the [`oro_common_proc_macros`] crate.
//!
//! It also houses the tests for the procedural macros.
#![cfg_attr(not(test), no_std)]

#[cfg(test)]
mod tests;

pub use oro_common_proc_macros::*;

/// Allows the unit variants of an enum to be iterated over.
///
/// This trait is derived via `#[derive(EnumIterator)]`.
/// See [`EnumIterator`] for more information.
pub trait EnumIterator: Copy + Sized {
	/// Returns an iterator over all unit variants of the enum.
	fn iter_all() -> impl Iterator<Item = Self> + Sized + 'static;
}
