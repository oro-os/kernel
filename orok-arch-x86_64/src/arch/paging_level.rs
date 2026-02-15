//! Types and utilities for working with x86_64 paging levels.

use orok_test::effect;
use orok_type::RelaxedUsize;

use crate::arch::reg;

/// The current paging level of the CPU.
#[expect(
	clippy::as_conversions,
	reason = "usize cast is safe as that is the repr of the enum"
)]
static CURRENT_PAGING_LEVEL: RelaxedUsize = RelaxedUsize::new(PagingLevel::Level4 as usize);

/// The number of levels in the page table hierarchy,
/// as determined by the CPU flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
#[expect(
	clippy::exhaustive_enums,
	reason = "these are the only supported paging levels on x86_64"
)]
pub enum PagingLevel {
	/// 4-level paging.
	Level4 = 4,
	/// 5-level paging.
	Level5 = 5,
}

impl PagingLevel {
	/// Returns the number of levels in the page table hierarchy.
	#[expect(clippy::inline_always, reason = "simple usize conversion")]
	#[inline(always)]
	#[expect(
		clippy::as_conversions,
		reason = "usize cast is safe as that is the repr of the enum"
	)]
	#[must_use]
	pub const fn as_usize(self) -> usize {
		self as usize
	}

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

	/// Returns the current paging level.
	///
	/// This function reads from a cached value, so it is faster
	/// than `current_from_cpu()`. However, it may be stale if the
	/// paging level has changed since the last time it was updated.
	#[cfg_attr(
		not(debug_assertions),
		expect(clippy::inline_always, reason = "simple load from atomic + match")
	)]
	#[cfg_attr(debug_assertions, inline(never))]
	#[cfg_attr(not(debug_assertions), inline(always))]
	#[must_use]
	#[effect(read_cache = CURRENT_PAGING_LEVEL)]
	pub fn current() -> Self {
		#[cfg(debug_assertions)]
		#[expect(
			clippy::unreachable,
			reason = "the stored value should always be valid"
		)]
		{
			// In debug mode, verify that the cached value matches the CPU value.
			let cpu_level = Self::current_from_cpu();
			let cached_level = match CURRENT_PAGING_LEVEL.load() {
				4 => Self::Level4,
				5 => Self::Level5,
				_ => unreachable!("invalid paging level stored in CURRENT_PAGING_LEVEL"),
			};
			debug_assert_eq!(
				cpu_level, cached_level,
				"cached paging level does not match CPU paging level"
			);
		}

		#[expect(
			clippy::unreachable,
			reason = "the stored value should always be valid"
		)]
		match CURRENT_PAGING_LEVEL.load() {
			4 => Self::Level4,
			5 => Self::Level5,
			_ => unreachable!("invalid paging level stored in CURRENT_PAGING_LEVEL"),
		}
	}

	/// Updates the virtual address masks based on the current paging level.
	///
	/// # Safety
	/// This function modifies global state related to virtual address
	/// translation. All virtual addresses previously validated may become
	/// invalid after calling this function.
	#[inline]
	#[effect(write_cache = CURRENT_PAGING_LEVEL)]
	pub unsafe fn refresh_globals() {
		let level = Self::current_from_cpu();
		CURRENT_PAGING_LEVEL.store(level.as_usize());
	}

	/// Returns the number of bits used for virtual addresses
	/// based on the current paging level.
	#[expect(clippy::inline_always, reason = "simple match")]
	#[expect(unused, reason = "temporarily unused")]
	#[inline(always)]
	#[must_use]
	pub const fn virtual_address_bits(self) -> usize {
		match self {
			Self::Level4 => 48,
			Self::Level5 => 57,
		}
	}

	/// Returns the mask for valid virtual addresses
	/// based on the current paging level.
	#[expect(clippy::inline_always, reason = "simple match")]
	#[expect(unused, reason = "temporarily unused")]
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
		// SAFETY: This is just testing
		unsafe {
			PagingLevel::refresh_globals();
		}
		assert_eq!(PagingLevel::current(), PagingLevel::Level4);
		reg::Cr4::load().with_la57(true).store();
		// SAFETY: This is just testing
		unsafe {
			PagingLevel::refresh_globals();
		}
		assert_eq!(PagingLevel::current(), PagingLevel::Level5);
	}

	#[test]
	#[should_panic(expected = "cached paging level does not match CPU paging level")]
	#[cfg_attr(
		not(debug_assertions),
		ignore = "cannot trigger unreachable in release mode"
	)]
	fn test_invalid_paging_level_check() {
		reg::Cr4::load().with_la57(false).store();
		// SAFETY: This is just testing
		unsafe {
			PagingLevel::refresh_globals();
		}
		assert_eq!(PagingLevel::current(), PagingLevel::Level4);
		reg::Cr4::load().with_la57(true).store();
		// This should trigger a debug assertion failure
		let _ = PagingLevel::current();
	}
}
