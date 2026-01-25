#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

use core::{fmt::Debug, hash::Hash};

use orok_macro::blanket_trait as blanket;

/// An architecture. All associated types / constants must be specified.
pub trait Arch {
	/// The page sizes available on the architecture.
	type PageSize: PageSize;
	/// See [`UnsafePhys`].
	type UnsafePhys: UnsafePhys;
	/// See [`UnsafeVirt`].
	type UnsafeVirt: UnsafeVirt;
}

/// An unsafe physical address type for the architecture.
#[blanket]
pub trait UnsafePhys:
	Sized + Copy + Debug + CheckUnsafePhys + PartialEq + Eq + PartialOrd + Ord + Hash + 'static
{
}

/// An unsafe virtual address type for the architecture.
#[blanket]
pub trait UnsafeVirt:
	Sized + Copy + Debug + CheckUnsafeVirt + PartialEq + Eq + PartialOrd + Ord + Hash + 'static
{
}

/// Trait representing page sizes available on the architecture.
pub trait PageSize: Sized + 'static {
	/// Returns the page size's size in bytes.
	///
	/// This is also treated as the alignment.
	fn page_size_bytes(&self) -> usize;
}

/// Checks that an [`UnsafePhys`] is valid.
pub trait CheckUnsafePhys {
	/// The error type returned when validation fails.
	type Error;

	/// Checks whether the given unsafe physical address is valid.
	fn check_phys(self) -> Result<(), Self::Error>;
}

/// Checks that an [`UnsafeVirt`] is valid.
pub trait CheckUnsafeVirt {
	/// The error type returned when validation fails.
	type Error;

	/// Checks whether the given unsafe virtual address is valid.
	fn check_virt(self) -> Result<(), Self::Error>;
}
