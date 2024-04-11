//! The Oro kernel supports x86_64's 4-level and 5-level paging modes,
//! which are determined by the CPU flags and conveyed to certain algorithms
//! with the [`PagingLevel`] enum.
#![allow(clippy::inline_always)]

/// The number of levels in the page table hierarchy,
/// as determined by the CPU flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PagingLevel {
	/// 4-level paging
	Level4 = 4,
	/// 5-level paging
	Level5 = 5,
}

impl PagingLevel {
	/// Returns the number of levels in the page table hierarchy.
	#[inline(always)]
	#[must_use]
	pub fn as_usize(self) -> usize {
		self as usize
	}

	/// Returns the current paging level based on CPU register flags.
	#[inline]
	#[cold]
	#[must_use]
	pub fn current_from_cpu() -> Self {
		if crate::asm::is_5_level_paging_enabled() {
			Self::Level5
		} else {
			Self::Level4
		}
	}
}
