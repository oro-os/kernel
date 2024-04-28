//! Hosts a collection of synchronization primitives that are used throughout the
//! kernel.

pub(crate) mod barrier;
pub(crate) mod spinlock;

pub use self::{
	barrier::SpinBarrier,
	spinlock::{
		unfair::{UnfairSpinlock, UnfairSpinlockGuard},
		unfair_critical::{UnfairCriticalSpinlock, UnfairCriticalSpinlockGuard},
	},
};
