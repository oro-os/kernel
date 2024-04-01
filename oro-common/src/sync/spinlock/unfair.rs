//! Provides the [`UnfairSpinlock`] type, a simple spinlock that does not
//! guarantee fairness and may result in starvation.

#![allow(clippy::module_name_repetitions)]

use core::{
	cell::UnsafeCell,
	marker::PhantomData,
	sync::atomic::{AtomicBool, Ordering},
};

use crate::Arch;

/// The unfair spinlock is a simple spinlock that does not guarantee
/// fairness and may result in starvation. It is used in the kernel for its
/// simplicity and low overhead.
///
/// Note that this implementation does _not_ put the system into a critical section.
/// If that behavior is desired, consider using an [`crate::sync::UnfairCriticalSpinlock`]
/// instead.
pub struct UnfairSpinlock<A: Arch, T> {
	/// Whether the lock is currently owned.
	owned: AtomicBool,
	/// The value protected by the lock.
	value: UnsafeCell<T>,
	/// The architecture this spinlock is for.
	_arch: PhantomData<A>,
}

unsafe impl<A: Arch, T> Sync for UnfairSpinlock<A, T> {}

impl<A: Arch, T> UnfairSpinlock<A, T> {
	/// Creates a new `UnfairSpinlock`.
	#[inline]
	pub const fn new(value: T) -> Self {
		Self {
			owned: AtomicBool::new(false),
			value: UnsafeCell::new(value),
			_arch: PhantomData,
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
		A::strong_memory_barrier();

		self.owned
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.ok()
			.map(|_| UnfairSpinlockGuard {
				lock: &self.owned,
				value: self.value.get(),
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
	lock: &'a AtomicBool,
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

impl<A: Arch, T> Default for UnfairSpinlock<A, T>
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
