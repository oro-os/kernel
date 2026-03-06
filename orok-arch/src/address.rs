//! Architecture-specific address types.

use crate::{CheckUnsafePhys, CheckUnsafeVirt, UnsafePhys, UnsafeVirt};

/// A valid physical address.
///
/// The architecture ensures that all [`Phys`]'s are valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Phys(UnsafePhys);

impl Phys {
	/// Creates a new validated [`Phys`] from an [`UnsafePhys`].
	///
	/// # Errors
	/// Returns an error if the address is not valid for the architecture.
	#[inline]
	pub fn new(value: UnsafePhys) -> Result<Self, <UnsafePhys as CheckUnsafePhys>::Error> {
		value.check_phys()?;
		Ok(Self(value))
	}
}

/// A valid virtual address.
///
/// The architecture ensures that all [`Virt`]'s are valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Virt(UnsafeVirt);

impl Virt {
	/// Creates a new validated [`Virt`] from an [`UnsafeVirt`].
	///
	/// # Errors
	/// Returns an error if the address is not valid for the architecture.
	#[inline]
	pub fn new(value: UnsafeVirt) -> Result<Self, <UnsafeVirt as CheckUnsafeVirt>::Error> {
		value.check_virt()?;
		Ok(Self(value))
	}
}
