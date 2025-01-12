//! Synchronization primitives for the Oro Kernel.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

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
pub trait Lock {
	/// The target type of value being guarded.
	type Target: Send + 'static;

	/// The lock guard type used by the lock implementation.
	type Guard<'a>: Drop + Deref<Target = Self::Target> + DerefMut
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

impl<T: Send + 'static> Lock for Mutex<T> {
	type Guard<'a> = MutexGuard<'a, T>;
	type Target = T;

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

impl<T: Default + Send + 'static> Default for Mutex<T> {
	fn default() -> Self {
		Self::new(T::default())
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

impl<T: Send + 'static> Lock for TicketMutex<T> {
	type Guard<'a> = TicketMutexGuard<'a, T>;
	type Target = T;

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

impl<T: Default + Send + 'static> Default for TicketMutex<T> {
	fn default() -> Self {
		Self::new(T::default())
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

#[doc(hidden)]
#[cfg(feature = "reentrant_mutex")]
mod reentrant {
	use core::{
		cell::UnsafeCell,
		sync::atomic::{
			AtomicU64,
			Ordering::{Acquire, Relaxed, Release},
		},
	};

	use super::Lock;

	unsafe extern "C" {
		/// Returns the current core ID.
		///
		/// # Safety
		/// This function must always return a valid core ID for the currently
		/// running execution context. If this value has not yet been set up,
		/// reentrant mutexes should NOT be locked (though they can be created).
		///
		/// The value returned by this function MUST always be the same for the same
		/// core.
		unsafe fn oro_sync_current_core_id() -> u32;
	}

	/// A reentrant mutex implementation.
	///
	/// This mutex allows the same core to lock the mutex multiple times.
	///
	/// **NOTE:** This implementation spins (and does not lock) if the refcount
	/// reaches `u32::MAX`. This is usually not a problem.
	pub struct ReentrantMutex<T: Send + 'static> {
		/// The inner value.
		inner: UnsafeCell<T>,
		/// The lock state.
		///
		/// The upper 32 bits are the core ID of the lock holder, and the lower 32 bits
		/// are the lock count.
		lock:  AtomicU64,
	}

	impl<T: Send + 'static> ReentrantMutex<T> {
		/// Constructs a new reentrant mutex.
		pub const fn new(inner: T) -> Self {
			Self {
				inner: UnsafeCell::new(inner),
				lock:  AtomicU64::new(0),
			}
		}
	}

	impl<T: Send + 'static> Lock for ReentrantMutex<T> {
		/// The lock guard type used by the lock implementation.
		type Guard<'a> = ReentrantMutexGuard<'a, Self::Target>;
		/// The target type of value being guarded.
		type Target = T;

		fn lock(&self) -> Self::Guard<'_> {
			// SAFETY: The safety requirements for this function are offloaded to the
			// SAFETY: implementation; it's marked unsafe as a requirement by Rust.
			let core_id = unsafe { oro_sync_current_core_id() };

			loop {
				let current = self.lock.load(Acquire);
				let current_core = (current >> 32) as u32;
				let current_count = (current & 0xFFFF_FFFF) as u32;

				if (current == 0 || current_core == core_id)
					&& self
						.lock
						.compare_exchange_weak(
							current,
							(u64::from(core_id) << 32) | u64::from(current_count + 1),
							Release,
							Relaxed,
						)
						.is_ok()
				{
					return ReentrantMutexGuard { inner: self };
				}
			}
		}
	}

	/// A guard for a reentrant mutex.
	pub struct ReentrantMutexGuard<'a, T: Send + 'static> {
		inner: &'a ReentrantMutex<T>,
	}

	impl<T: Send + 'static> core::ops::Deref for ReentrantMutexGuard<'_, T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			// SAFETY: The guard is only created if the lock is held.
			unsafe { &*self.inner.inner.get() }
		}
	}

	impl<T: Send + 'static> core::ops::DerefMut for ReentrantMutexGuard<'_, T> {
		fn deref_mut(&mut self) -> &mut Self::Target {
			// SAFETY: The guard is only created if the lock is held.
			unsafe { &mut *self.inner.inner.get() }
		}
	}

	impl<T: Send + 'static> Drop for ReentrantMutexGuard<'_, T> {
		fn drop(&mut self) {
			loop {
				let current = self.inner.lock.load(Relaxed);
				let current_count = current & 0xFFFF_FFFF;

				debug_assert_eq!(
					(current >> 32) as u32,
					unsafe { oro_sync_current_core_id() },
					"re-entrant lock held lock by another core upon drop"
				);

				if self
					.inner
					.lock
					.compare_exchange(
						current,
						if current_count == 1 { 0 } else { current - 1 },
						Release,
						Relaxed,
					)
					.is_ok()
				{
					break;
				}
			}
		}
	}

	unsafe impl<T: Send + 'static> Sync for ReentrantMutex<T> {}
}

#[cfg(feature = "reentrant_mutex")]
pub use reentrant::*;
