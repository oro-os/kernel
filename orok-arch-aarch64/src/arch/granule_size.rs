//! Implements granule sizes caches the current granule size of the system.

use crate::arch::reg;

/// Different granule sizes for the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum GranuleSize {
	/// 4 KiB granule size.
	Size4KiB  = 4,
	/// 16 KiB granule size.
	Size16KiB = 16,
	/// 64 KiB granule size.
	Size64KiB = 64,
}

impl GranuleSize {
	/// Returns the granule size in bytes.
	#[must_use = "this function has no side effects; it makes no sense to discard the return value"]
	pub const fn size_bytes(self) -> usize {
		match self {
			Self::Size4KiB => 4 * 1024,
			Self::Size16KiB => 16 * 1024,
			Self::Size64KiB => 64 * 1024,
		}
	}

	/// Returns the granule size in 4096-byte units (minimum page size).
	#[must_use = "this function has no side effects; it makes no sense to discard the return value"]
	pub const fn size_in_4kib_units(self) -> usize {
		match self {
			Self::Size4KiB => 1,
			Self::Size16KiB => 4,
			Self::Size64KiB => 16,
		}
	}

	/// Returns the current granule size of the supervisor (EL1),
	/// as determined by the CPU flags.
	///
	/// Returns `None` if the granule size as reported by the
	/// CPU is invalid.
	///
	/// # Important
	/// This function reads the CPU register flags to determine
	/// the current granule size. It should only be used if the
	/// granule size may have changed since the last time it was checked.
	///
	/// It should be considered slower than `current()`.
	#[inline]
	#[cold]
	#[must_use = "this function has no side effects; it makes no sense to discard the return value"]
	pub fn current_from_cpu_br1_el1() -> Option<Self> {
		reg::TcrEl1::load().tg1().map(Into::into)
	}
}

impl From<reg::tcr_el1::Tg1GranuleSize> for GranuleSize {
	fn from(value: reg::tcr_el1::Tg1GranuleSize) -> Self {
		use reg::tcr_el1::Tg1GranuleSize;
		match value {
			Tg1GranuleSize::Kb4 => Self::Size4KiB,
			Tg1GranuleSize::Kb16 => Self::Size16KiB,
			Tg1GranuleSize::Kb64 => Self::Size64KiB,
		}
	}
}

impl From<GranuleSize> for reg::tcr_el1::Tg1GranuleSize {
	fn from(value: GranuleSize) -> Self {
		match value {
			GranuleSize::Size4KiB => Self::Kb4,
			GranuleSize::Size16KiB => Self::Kb16,
			GranuleSize::Size64KiB => Self::Kb64,
		}
	}
}
