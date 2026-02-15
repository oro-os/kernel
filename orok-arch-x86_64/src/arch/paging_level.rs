//! Types and utilities for working with x86_64 paging levels.

use crate::arch::reg;

/// The number of levels in the page table hierarchy,
/// as determined by the CPU flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PagingLevel {
	/// 4-level paging.
	Level4 = 4,
	/// 5-level paging.
	Level5 = 5,
}

impl PagingLevel {
	/// Returns the current paging level based on CPU register flags.
	///
	/// # Important
	/// This function reads the CPU register flags to determine
	/// the current paging level. It should only be used if the
	/// paging level may have changed since the last time it was checked.
	///
	/// It should be considered slower than `current()`.
	#[inline]
	#[cold]
	#[must_use]
	pub fn current_from_cpu() -> Self {
		if reg::Cr4::load().la57() {
			Self::Level5
		} else {
			Self::Level4
		}
	}

	/// Returns the number of bits used for virtual addresses
	/// for this paging level.
	#[expect(clippy::inline_always, reason = "simple match")]
	#[inline(always)]
	#[must_use]
	pub const fn virtual_address_bits(self) -> usize {
		match self {
			Self::Level4 => 48,
			Self::Level5 => 57,
		}
	}

	/// Returns the mask for valid virtual addresses for this
	/// paging level.
	#[expect(clippy::inline_always, reason = "simple match")]
	#[inline(always)]
	#[must_use]
	pub const fn virtual_address_mask(self) -> u64 {
		match self {
			Self::Level4 => 0x0000_FFFF_FFFF_FFFF,
			Self::Level5 => 0x01FF_FFFF_FFFF_FFFF,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cr4_load() {
		reg::Cr4::load().with_la57(false).store();
		assert_eq!(PagingLevel::current_from_cpu(), PagingLevel::Level4);
		reg::Cr4::load().with_la57(true).store();
		assert_eq!(PagingLevel::current_from_cpu(), PagingLevel::Level5);
		reg::Cr4::load().with_la57(false).store();
		assert_eq!(PagingLevel::current_from_cpu(), PagingLevel::Level4);
	}
}
