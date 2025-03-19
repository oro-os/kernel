//! Implements timekeeping facilities for the x86_64 platform.

use core::time::Duration;

use oro_kernel::arch::InstantResult;

/// The numeric type of `femtoseconds_per_tick` in [`calculate_fsns_magic`].
pub type FsNsInt = u32;

/// The factor to multiply/divide by to convert
/// femtoseconds to nanoseconds.
///
/// This is `ceil(log2(1,000,000))` plus the bit width of [`FsNsInt`].
pub const FS_NS_SHIFT: u32 = 20 + FsNsInt::BITS;

/// Calculates a magic multiplier value for a femtoseconds-per-tick
/// value that can be used to multiply against a tick count such that
/// the result can be right-shifted by [`FS_NS_SHIFT`] to arrive at
/// an accurate nanosecond count.
#[must_use]
pub const fn calculate_fsns_magic(femtoseconds_per_tick: FsNsInt) -> u128 {
	const M: u128 = const { 2u128.pow(FS_NS_SHIFT).div_ceil(1_000_000) };
	(femtoseconds_per_tick as u128) * M
}

/// The instant used by the x86_64 cores.
///
/// Holds a nanosecond divider and a large counter.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct Instant {
	/// The timestamp, in femtoseconds.
	time: u128,
}

impl Instant {
	/// Creates a new `Instant` given the femtosecond timer
	#[must_use]
	#[inline]
	pub(crate) const fn new(time: u128) -> Self {
		Self { time }
	}
}

impl oro_kernel::arch::Instant for Instant {
	fn checked_add(&self, duration: &Duration) -> InstantResult<Self> {
		// NOTE(qix-): Under normal circumstances, this is obviously
		// NOTE(qix-): overkill. Femtoseconds in a u128 will only wrap
		// NOTE(qix-): after the age of the universe times 780,000.
		// NOTE(qix-): That doesn't preclude malicious actors doing time
		// NOTE(qix-): bending or some such. So let's be safe anyway.
		#[expect(
			clippy::useless_conversion,
			reason = "just to make sure `as_nanos()` doesn't change type in the future"
		)]
		let (v, wrapped) = self
			.time
			.overflowing_add(u128::from(duration.as_nanos()) << FS_NS_SHIFT);
		if wrapped {
			InstantResult::Overflow(Instant { time: v })
		} else {
			InstantResult::Ok(Instant { time: v })
		}
	}

	fn checked_duration_since(&self, other: &Self) -> Option<Duration> {
		self.time
			.checked_sub(other.time)
			.and_then(|v| Some(Duration::from_nanos(u64::try_from(v >> FS_NS_SHIFT).ok()?)))
	}
}

/// A type that can generate/retrieve [`Instant`]s.
pub trait GetInstant: Sync + Send {
	/// Returns the current [`Instant`].
	///
	/// See [`oro_kernel::arch::CoreHandle::now()`] for more information.
	fn now(&self) -> Instant;
}
