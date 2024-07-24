//! Barrier types, used to synchronize multiple cores in lockstep.

#![allow(clippy::module_name_repetitions)]

use crate::{unsafe_precondition, Arch};
use core::sync::atomic::{AtomicU64, Ordering};

/// An atomic, spin-based barrier used to synchronize multiprocessor
/// operations. This barrier is a bit different than other barriers
/// because it has two phases; the first is to wait for a total number
/// (that should come from exactly one core), and the second is to
/// wait for all cores to reach the barrier.
///
/// This barrier is single-use.
pub struct SpinBarrier {
	/// The number of cores that must reach the barrier before it is cleared.
	count: AtomicU64,
	/// The total number of cores that must reach the barrier before it is cleared.
	/// Zero indicates that a total has not been set.
	total: AtomicU64,
}

impl SpinBarrier {
	/// Creates a new `SpinBarrier` that will synchronize `total` cores.
	#[allow(clippy::new_without_default)]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			count: AtomicU64::new(0),
			total: AtomicU64::new(0),
		}
	}

	/// Sets the total of the barrier. This does not increment the count
	/// nor does it block; the caller must also call `wait()`.
	///
	/// # Safety
	/// Consumers must ensure that this function is called exactly ONCE
	/// from a SINGLE core.
	pub unsafe fn set_total<A: Arch>(&self, total: u64) {
		unsafe_precondition!(
			A,
			self.total.load(Ordering::Acquire) == 0,
			"total already set"
		);
		self.total.store(total, Ordering::Release);
	}

	/// Waits at the barrier until all cores have reached it.
	pub fn wait(&self) {
		let mut total;
		loop {
			total = self.total.load(Ordering::Acquire);

			if total > 0 {
				break;
			}

			::core::hint::spin_loop();
		}

		let count = self.count.fetch_add(1, Ordering::Acquire) + 1;

		if count == total {
			self.count.store(0, Ordering::Release);
		} else {
			while self.count.load(Ordering::Acquire) != 0 {
				::core::hint::spin_loop();
			}
		}
	}
}
