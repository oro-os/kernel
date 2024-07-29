//! Provides the [`UnfairSpinlock`] type, a simple spinlock that does not
//! guarantee fairness and may result in starvation.

#![allow(clippy::module_name_repetitions)]

use crate::Arch;
use core::{
	cell::UnsafeCell,
	sync::atomic::{AtomicBool, Ordering},
};

/// The unfair spinlock is a simple spinlock that does not guarantee
/// fairness and may result in starvation. It is used in the kernel for its
/// simplicity and low overhead.
///
/// Note that this implementation does _not_ put the system into a critical section.
/// If that behavior is desired, consider using an [`crate::sync::UnfairCriticalSpinlock`]
/// instead.
pub struct UnfairSpinlock<T> {
	/// Whether the lock is currently owned.
	owned: AtomicBool,
	/// The value protected by the lock.
	value: UnsafeCell<T>,
}

unsafe impl<T> Sync for UnfairSpinlock<T> {}

impl<T> UnfairSpinlock<T> {
	/// Creates a new `UnfairSpinlock`.
	#[inline]
	pub const fn new(value: T) -> Self {
		Self {
			owned: AtomicBool::new(false),
			value: UnsafeCell::new(value),
		}
	}

	/// Attempts to acquire the lock.
	///
	/// If the lock is currently owned by another core, this method will return `None`.
	///
	/// # Safety
	/// This function is not reentrant.
	#[inline]
	#[must_use]
	pub unsafe fn try_lock<A: Arch>(&self) -> Option<UnfairSpinlockGuard<T>> {
		A::strong_memory_barrier();

		self.owned
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.ok()
			.map(|_| {
				UnfairSpinlockGuard {
					lock:  &self.owned,
					value: self.value.get(),
				}
			})
	}

	/// Locks the spinlock, blocking until it is acquired.
	///
	/// # Safety
	/// This function is not reentrant.
	#[inline]
	#[must_use]
	pub unsafe fn lock<A: Arch>(&self) -> UnfairSpinlockGuard<T> {
		loop {
			if let Some(guard) = self.try_lock::<A>() {
				return guard;
			}
		}
	}
}

/// A lock held by an [`UnfairSpinlock`].
pub struct UnfairSpinlockGuard<'a, T> {
	/// A handle to the 'owned' flag in the spinlock.
	lock:  &'a AtomicBool,
	/// A pointer to the value protected by the spinlock.
	value: *mut T,
}

impl<T> Drop for UnfairSpinlockGuard<'_, T> {
	#[inline]
	fn drop(&mut self) {
		// NOTE(qix-): Please do not re-order. It is important
		// NOTE(qix-): that the interrupt state is restored after
		// NOTE(qix-): the lock is released, as there may be
		// NOTE(qix-): an interrupt that comes in between the
		// NOTE(qix-): lock release and the interrupt state
		// NOTE(qix-): restoration, causing starvation of other cores
		// NOTE(qix-): for the duration of the interrupt handler.
		self.lock.store(false, Ordering::Release);
	}
}

impl<T> Default for UnfairSpinlock<T>
where
	T: Default,
{
	#[inline]
	fn default() -> Self {
		Self::new(Default::default())
	}
}

impl<T> core::ops::Deref for UnfairSpinlockGuard<'_, T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { &*self.value }
	}
}

impl<T> core::ops::DerefMut for UnfairSpinlockGuard<'_, T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.value }
	}
}
