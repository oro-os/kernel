//! Implements unsafe address types for AArch64.
//!
//! These are used for physical and virtual addresses that may
//! not be valid, and thus cannot be safely dereferenced.

use orok_arch_base::{CheckUnsafePhys, CheckUnsafeVirt};

/// An unsafe physical address type for the architecture.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UnsafePhys(u64);

impl From<u64> for UnsafePhys {
	#[inline]
	fn from(value: u64) -> Self {
		Self(value)
	}
}

impl From<UnsafePhys> for u64 {
	#[inline]
	fn from(value: UnsafePhys) -> Self {
		value.0
	}
}

/// An unsafe virtual address type for the architecture.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UnsafeVirt(u64);
impl From<u64> for UnsafeVirt {
	#[inline]
	fn from(value: u64) -> Self {
		Self(value)
	}
}

impl From<UnsafeVirt> for u64 {
	#[inline]
	fn from(value: UnsafeVirt) -> Self {
		value.0
	}
}

/// Error type for validating physical addresses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PhysError {}

impl CheckUnsafePhys for UnsafePhys {
	type Error = PhysError;

	fn check_phys(self) -> Result<(), Self::Error> {
		todo!("validate physical address");
	}
}

/// Error type for validating virtual addresses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VirtError {
	/// One or more of the upper bits do not match the highest canonical bit
	/// (the address is not sign-extended).
	NonCanonicalAddress,
}

impl CheckUnsafeVirt for UnsafeVirt {
	type Error = VirtError;

	fn check_virt(self) -> Result<(), Self::Error> {
		todo!("validate virtual address");
	}
}

impl orok_arch_base::ArchAddressScheme for super::Arch {
	type UnsafePhys = UnsafePhys;
	type UnsafeVirt = UnsafeVirt;
}
