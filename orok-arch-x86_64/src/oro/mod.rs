//! Oro-specific x86_64 architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

use orok_arch_base::{Arch as BaseArch, CheckUnsafePhys, CheckUnsafeVirt};

use crate::arch::PagingLevel;

/// Implements the x86_64 architecture.
#[non_exhaustive]
pub struct Arch;

impl BaseArch for Arch {
	type PageSize = PageSize;
	type UnsafePhys = UnsafePhys;
	type UnsafeVirt = UnsafeVirt;
}

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
#[expect(
	clippy::exhaustive_enums,
	reason = "this is an arch-specific error type; stability is not necessary"
)]
pub enum PhysError {
	/// One or more of the upper 12 bits are set.
	Upper12BitsSet,
}

/// The available page sizes on x86_64.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[expect(
	clippy::arbitrary_source_item_ordering,
	reason = "ordered from smallest to largest page size"
)]
#[expect(
	clippy::exhaustive_enums,
	reason = "this is an arch-specific error type; stability is not necessary"
)]
pub enum PageSize {
	/// 4 KiB page size.
	Size4KiB,
	/// 2 MiB page size.
	Size2MiB,
	/// 1 GiB page size.
	Size1GiB,
}

impl orok_arch_base::PageSize for PageSize {
	#[inline]
	fn page_size_bytes(&self) -> usize {
		match *self {
			Self::Size4KiB => 4 * 1024,
			Self::Size2MiB => 2 * 1024 * 1024,
			Self::Size1GiB => 1024 * 1024 * 1024,
		}
	}
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
#[expect(
	clippy::exhaustive_enums,
	reason = "this is an arch-specific error type; stability is not necessary"
)]
pub enum VirtError {
	/// One or more of the upper bits do not match the highest canonical bit
	/// (the address is not sign-extended).
	NonCanonicalAddress,
}

impl CheckUnsafeVirt for UnsafeVirt {
	type Error = VirtError;

	#[expect(
		clippy::missing_inline_in_public_items,
		reason = "LTO can decide if this should be inlined or not"
	)]
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
