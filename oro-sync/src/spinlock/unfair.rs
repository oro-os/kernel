//! Provides the [`UnfairSpinlock`] type, a simple spinlock that does not
//! guarantee fairness and may result in starvation.

#![expect(clippy::module_name_repetitions)]

use core::{
	cell::UnsafeCell,
	sync::atomic::{AtomicBool, Ordering},
};

/// The unfair spinlock is a simple spinlock that does not guarantee
/// fairness and may result in starvation. It is used in the kernel for its
/// simplicity and low overhead.
///
/// Note that this implementation does _not_ put the system into a critical section.
/// If that behavior is desired, consider using an
/// [`crate::spinlock::unfair_critical::UnfairCriticalSpinlock`] instead.
pub struct UnfairSpinlock<T> {
	/// The value protected by the lock.
	value: UnsafeCell<T>,
	/// Whether the lock is currently owned.
	owned: AtomicBool,
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
	pub unsafe fn try_lock(&self) -> Option<UnfairSpinlockGuard<T>> {
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
	pub unsafe fn lock(&self) -> UnfairSpinlockGuard<T> {
		loop {
			if let Some(guard) = self.try_lock() {
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
