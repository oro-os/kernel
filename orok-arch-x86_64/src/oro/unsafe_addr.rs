//! Implements unsafe address types (virtual and physical) for x86_64.

use orok_arch_base::{CheckUnsafePhys, CheckUnsafeVirt};

use crate::arch::PagingLevel;

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
pub enum PhysError {
	/// One or more of the upper 12 bits are set.
	Upper12BitsSet,
}

impl CheckUnsafePhys for UnsafePhys {
	type Error = PhysError;

	#[inline]
	fn check_phys(self) -> Result<(), Self::Error> {
		/// The physical address mask for x86_64.
		const PHYS_ADDR_MASK: u64 = (1 << 52) - 1;

		// Only the lower 52 bits are valid.
		if self.0 & !PHYS_ADDR_MASK != 0 {
			return Err(PhysError::Upper12BitsSet);
		}

		Ok(())
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
		// Checks that the address is canonical (sign-extended).
		// This checks the processor flags to see if virtual addresses are 48 or 57 bits.
		const CANONICAL_MASK_48: u64 = 0x0000_FFFF_FFFF_FFFF;
		const CANONICAL_MASK_57: u64 = 0x01FF_FFFF_FFFF_FFFF;

		let paging_level = PagingLevel::current_from_cpu();
		let (upper_mask, high_bit) = match paging_level {
			PagingLevel::Level4 => (!CANONICAL_MASK_48, self.0 & (1 << 47) != 0),
			PagingLevel::Level5 => (!CANONICAL_MASK_57, self.0 & (1 << 56) != 0),
		};

		let is_canonical = if high_bit {
			(self.0 & upper_mask) == upper_mask
		} else {
			(self.0 & upper_mask) == 0
		};

		if !is_canonical {
			return Err(VirtError::NonCanonicalAddress);
		}

		// Get the alignment of the address.
		Ok(())
	}
}
