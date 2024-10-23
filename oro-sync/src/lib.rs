//! Synchronization primitives for the Oro Kernel.
#![cfg_attr(not(test), no_std)]

use core::{
	cell::UnsafeCell,
	ops::{Deref, DerefMut},
	sync::atomic::{
		AtomicBool, AtomicUsize,
		Ordering::{AcqRel, Acquire, Relaxed, Release},
	},
};

/// The number of iterations to wait for a stale ticket mutex lock.
const TICKET_MUTEX_TIMEOUT: usize = 1000;

/// Standardized lock interface implemented for all lock types.
pub trait Lock<T: Send + 'static> {
	/// The lock guard type used by the lock implementation.
	type Guard<'a>: Drop + Deref + DerefMut
	where
		Self: 'a;

	/// Acquires a lock, blocking until it's available.
	fn lock(&self) -> Self::Guard<'_>;
}

/// A simple unfair, greedy spinlock. The most efficient spinlock
/// available in this library.
pub struct Mutex<T: Send + 'static> {
	/// The guarded value.
	value:  UnsafeCell<T>,
	/// Whether or not the lock is taken.
	locked: AtomicBool,
}

// SAFETY: We are implementing a safe interface around a mutex so we can assert `Sync`.
unsafe impl<T: Send + 'static> Sync for Mutex<T> {}

impl<T: Send + 'static> Mutex<T> {
	/// Creates a new spinlock mutex for the given value.
	pub const fn new(value: T) -> Self {
		Self {
			value:  UnsafeCell::new(value),
			locked: AtomicBool::new(false),
		}
	}
}

impl<T: Send + 'static> Lock<T> for Mutex<T> {
	type Guard<'a> = MutexGuard<'a, T>;

	fn lock(&self) -> Self::Guard<'_> {
		loop {
			if !self.locked.swap(true, Acquire) {
				#[cfg(debug_assertions)]
				::oro_dbgutil::__oro_dbgutil_lock_acquire(self.value.get() as usize);
				return MutexGuard { lock: self };
			}

			::core::hint::spin_loop();
		}
	}
}

/// A mutex guard for the simple [`Mutex`] type.
pub struct MutexGuard<'a, T: Send + 'static>
where
	Self: 'a,
{
	/// A reference to the lock for which we have a guard.
	lock: &'a Mutex<T>,
}

impl<T: Send + 'static> Drop for MutexGuard<'_, T> {
	fn drop(&mut self) {
		#[cfg(debug_assertions)]
		::oro_dbgutil::__oro_dbgutil_lock_release(self.lock.value.get() as usize);
		self.lock.locked.store(false, Release);
	}
}

impl<T: Send + 'static> Deref for MutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &*self.lock.value.get() }
	}
}

impl<T: Send + 'static> DerefMut for MutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &mut *self.lock.value.get() }
	}
}

/// A ticketed, fair mutex implementation.
pub struct TicketMutex<T: Send + 'static> {
	/// The guarded value.
	value:       UnsafeCell<T>,
	/// The currently served ticket.
	now_serving: AtomicUsize,
	/// The next ticket.
	next_ticket: AtomicUsize,
	/// Whether or not we've locked the lock.
	locked:      AtomicBool,
}

// SAFETY: We are implementing a safe interface around a mutex so we can assert `Sync`.
unsafe impl<T: Send + 'static> Sync for TicketMutex<T> {}

impl<T: Send + 'static> TicketMutex<T> {
	/// Creates a new ticket mutex.
	pub const fn new(value: T) -> Self {
		Self {
			value:       UnsafeCell::new(value),
			now_serving: AtomicUsize::new(0),
			next_ticket: AtomicUsize::new(0),
			locked:      AtomicBool::new(false),
		}
	}
}

impl<T: Send + 'static> Lock<T> for TicketMutex<T> {
	type Guard<'a> = TicketMutexGuard<'a, T>;

	fn lock(&self) -> Self::Guard<'_> {
		'new_ticket: loop {
			let ticket = self.next_ticket.fetch_add(1, Relaxed);
			let mut old_now_serving = self.now_serving.load(Acquire);

			// NOTE(qix-): The wrapping is intentional and desirable.
			#[allow(clippy::cast_possible_wrap)]
			{
				debug_assert!((ticket.wrapping_sub(old_now_serving) as isize) >= 0);
			}

			let mut timeout = TICKET_MUTEX_TIMEOUT;

			loop {
				let now_serving = self.now_serving.load(Acquire);

				// NOTE(qix-): The wrapping is intentional and desirable.
				#[expect(clippy::cast_possible_wrap)]
				let position = ticket.wrapping_sub(now_serving) as isize;

				if position == 0 && !self.locked.swap(true, AcqRel) {
					#[cfg(debug_assertions)]
					::oro_dbgutil::__oro_dbgutil_lock_acquire(self.value.get() as usize);
					return TicketMutexGuard { lock: self, ticket };
				}

				if position < 0 {
					// We've been forcibly skipped; obtain a new ticket
					// and start over.
					continue 'new_ticket;
				}

				// If the ticket has been advanced, then reset the timeout.
				if now_serving != old_now_serving {
					old_now_serving = now_serving;
					timeout = TICKET_MUTEX_TIMEOUT;
				} else if !self.locked.load(Acquire) {
					// NOTE(qix-): Only wraps in the case of a logic bug.
					// NOTE(qix-): Once we compare-exchange the new ticket number,
					// NOTE(qix-): the ticket should be changed on the next iteration,
					// NOTE(qix-): resetting the timer. If this invariant is false for
					// NOTE(qix-): some reason, then timeout will wrap which will cause
					// NOTE(qix-): a debug assertion indicating a bug.
					timeout -= 1;

					if timeout == 0 {
						// The existing ticket has timed out; forcibly un-deadlock it.
						// We don't care about the result here; if another thread already
						// updated it, we honor that; otherwise ours is guaranteed to succeed.
						let _ = self.now_serving.compare_exchange(
							now_serving,
							now_serving.wrapping_add(1),
							AcqRel,
							Relaxed,
						);
					}
				}

				::core::hint::spin_loop();
			}
		}
	}
}

/// A lock guard for a [`TicketMutex`].
pub struct TicketMutexGuard<'a, T: Send + 'static>
where
	Self: 'a,
{
	/// The lock we are guarding.
	lock:   &'a TicketMutex<T>,
	/// Our ticket
	ticket: usize,
}

impl<T: Send + 'static> Drop for TicketMutexGuard<'_, T> {
	fn drop(&mut self) {
		#[cfg(debug_assertions)]
		::oro_dbgutil::__oro_dbgutil_lock_release(self.lock.value.get() as usize);
		let _ = self.lock.now_serving.compare_exchange(
			self.ticket,
			self.ticket.wrapping_add(1),
			Release,
			Relaxed,
		);
		self.lock.locked.store(false, Release);
	}
}

impl<T: Send + 'static> Deref for TicketMutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &*self.lock.value.get() }
	}
}

impl<T: Send + 'static> DerefMut for TicketMutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &mut *self.lock.value.get() }
	}
}
