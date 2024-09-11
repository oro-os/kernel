//! Provides the [`UnfairCriticalSpinlock`] type, a simple spinlock that does not
//! guarantee fairness and may result in starvation, that also disables
//! interrupts for the lifetime of an acquired lock.

#![allow(clippy::module_name_repetitions)]

use core::{
	cell::UnsafeCell,
	marker::PhantomData,
	sync::atomic::{AtomicBool, Ordering},
};

/// Allows for an architecture-specific means of disabling and re-enabling
/// interrupts.
pub trait InterruptController: 'static {
	/// The interrupt state for the architecture.
	type InterruptState: Copy;

	/// Disables interrupts.
	fn disable_interrupts();

	/// Restores interrupts to the given state.
	fn restore_interrupts(state: Self::InterruptState);

	/// Fetches the current interrupt state.
	fn fetch_interrupts() -> Self::InterruptState;
}

/// The unfair critical spinlock is a simple spinlock that does not guarantee
/// fairness and may result in starvation. It is used in the kernel for its
///  simplicity and low overhead.
///
/// Note that this implementation **puts the system into a critical section**
/// when a lock is acquired, which is exited when the lock is dropped.
///
/// Thus, its locking methods are marked `unsafe`, as the code that acquires
/// the lock **must not panic** while the lock is held.
#[repr(C)]
pub struct UnfairCriticalSpinlock<T> {
	/// The value protected by the lock.
	value: UnsafeCell<T>,
	/// Whether the lock is currently owned.
	owned: AtomicBool,
}

unsafe impl<T> Sync for UnfairCriticalSpinlock<T> {}

impl<T> UnfairCriticalSpinlock<T> {
	/// Creates a new `UnfairCriticalSpinlock`.
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
	/// This method is unsafe because the code that acquires the lock **must not panic**
	/// while the lock is held.
	///
	/// This function is not reentrant.
	#[inline]
	#[must_use]
	pub unsafe fn try_lock<C: InterruptController>(
		&self,
	) -> Option<UnfairCriticalSpinlockGuard<C, T>> {
		self.owned
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.ok()
			.map(|_| {
				let interrupt_state = C::fetch_interrupts();
				C::disable_interrupts();

				UnfairCriticalSpinlockGuard {
					lock: &self.owned,
					value: self.value.get(),
					interrupt_state,
					_arch: PhantomData,
				}
			})
	}

	/// Locks the spinlock, blocking until it is acquired.
	///
	/// # Safety
	/// This method is unsafe because the code that acquires the lock **must not panic**
	/// while the lock is held.
	///
	/// This function is not reentrant.
	#[inline]
	#[must_use]
	pub unsafe fn lock<C: InterruptController>(&self) -> UnfairCriticalSpinlockGuard<C, T> {
		let interrupt_state = C::fetch_interrupts();
		C::disable_interrupts();

		loop {
			if let Some(guard) = self.try_lock_with_interrupt_state::<C>(interrupt_state) {
				return guard;
			}
		}
	}

	/// Tries to lock the spinlock, returning `None` if the lock is already held.
	///
	/// # Safety
	/// This method is unsafe because the code that acquires the lock **must not panic**.
	/// Further, interrupts should be properly fetched prior to disabling them.
	///
	/// This function is not reentrant.
	#[inline]
	#[must_use]
	unsafe fn try_lock_with_interrupt_state<C: InterruptController>(
		&self,
		interrupt_state: C::InterruptState,
	) -> Option<UnfairCriticalSpinlockGuard<C, T>> {
		self.owned
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.ok()
			.map(|_| {
				UnfairCriticalSpinlockGuard {
					lock: &self.owned,
					value: self.value.get(),
					interrupt_state,
					_arch: PhantomData,
				}
			})
	}
}

/// A lock held by an [`UnfairCriticalSpinlock`].
pub struct UnfairCriticalSpinlockGuard<'a, C: InterruptController, T> {
	/// The interrupt state before the lock was acquired.
	interrupt_state: C::InterruptState,
	/// A handle to the `owned` flag in the spinlock.
	lock: &'a AtomicBool,
	/// The value protected by the lock.
	value: *mut T,
	/// The architecture this spinlock guard is for.
	/// Used to restore interrupts on drop.
	_arch: PhantomData<C>,
}

impl<C: InterruptController, T> Drop for UnfairCriticalSpinlockGuard<'_, C, T> {
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
		C::restore_interrupts(self.interrupt_state);
	}
}

impl<T> Default for UnfairCriticalSpinlock<T>
where
	T: Default,
{
	#[inline]
	fn default() -> Self {
		Self::new(Default::default())
	}
}

impl<C: InterruptController, T> core::ops::Deref for UnfairCriticalSpinlockGuard<'_, C, T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { &*self.value }
	}
}

impl<C: InterruptController, T> core::ops::DerefMut for UnfairCriticalSpinlockGuard<'_, C, T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.value }
	}
}
