//! Traits for defining architecture-specific address schemes.

use orok_macro::blanket_trait as blanket;

/// Defines trait types for an architecture's address scheme.
pub trait ArchAddressScheme {
	/// An unsafe physical address type for the architecture.
	type UnsafePhys: UnsafePhys;
	/// An unsafe virtual address type for the architecture.
	type UnsafeVirt: UnsafeVirt;
}

/// An unsafe physical address type for the architecture.
#[blanket]
pub trait UnsafePhys:
	Sized
	+ Copy
	+ core::fmt::Debug
	+ CheckUnsafePhys
	+ PartialEq
	+ Eq
	+ PartialOrd
	+ Ord
	+ core::hash::Hash
	+ 'static
{
}

/// An unsafe virtual address type for the architecture.
#[blanket]
pub trait UnsafeVirt:
	Sized
	+ Copy
	+ core::fmt::Debug
	+ CheckUnsafeVirt
	+ PartialEq
	+ Eq
	+ PartialOrd
	+ Ord
	+ core::hash::Hash
	+ 'static
{
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
