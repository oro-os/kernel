//! Timekeeping implementation for the AArch64 architecture.

use core::time::Duration;

/// The instant used by the AArch64 cores.
#[derive(Debug, Eq, Clone, Copy)]
pub struct Instant;

impl oro_kernel::arch::Instant for Instant {
	fn checked_add(
		&self,
		_duration: &core::time::Duration,
	) -> oro_kernel::arch::InstantResult<Self> {
		todo!();
	}

	fn checked_duration_since(&self, _other: &Self) -> Option<Duration> {
		todo!();
	}
}

impl core::cmp::PartialOrd for Instant {
	fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl core::cmp::Ord for Instant {
	fn cmp(&self, _other: &Self) -> core::cmp::Ordering {
		todo!();
	}
}

impl core::cmp::PartialEq for Instant {
	fn eq(&self, _other: &Self) -> bool {
		todo!();
	}
}
