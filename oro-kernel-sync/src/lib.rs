//! Synchronization primitives for the Oro Kernel.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
// SAFETY(qix-): This is accepted but is taking ages to stabilize. In theory
// SAFETY(qix-): marker fields could be used but for now I want to keep things
// SAFETY(qix-): cleaner and more readable.
#![feature(negative_impls)]

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
	type Target: ?Sized;

	/// The lock guard type used by the lock implementation.
	type Guard<'a>: Drop + Deref<Target = Self::Target> + DerefMut
	where
		Self: 'a;

	/// Acquires a lock, blocking until it's available.
	fn lock(&self) -> Self::Guard<'_>;
}

/// A simple unfair, greedy spinlock. The most efficient spinlock
/// available in this library.
pub struct Mutex<T: ?Sized> {
	/// Whether or not the lock is taken.
	locked: AtomicBool,
	/// The guarded value.
	value:  UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
	/// Creates a new spinlock mutex for the given value.
	pub const fn new(value: T) -> Self {
		Self {
			value:  UnsafeCell::new(value),
			locked: AtomicBool::new(false),
		}
	}
}

impl<T: ?Sized> Lock for Mutex<T> {
	type Guard<'a>
		= MutexGuard<'a, T>
	where
		T: 'a;
	type Target = T;

	fn lock(&self) -> Self::Guard<'_> {
		loop {
			if !self.locked.swap(true, Acquire) {
				::oro_dbgutil::__oro_dbgutil_lock_acquire(self.value.get() as *const () as usize);
				return MutexGuard { lock: self };
			}

			::core::hint::spin_loop();
		}
	}
}

impl<T: Default> Default for Mutex<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

/// A mutex guard for the simple [`Mutex`] type.
pub struct MutexGuard<'a, T: ?Sized + 'a>
where
	Self: 'a,
{
	/// A reference to the lock for which we have a guard.
	lock: &'a Mutex<T>,
}

impl<T: ?Sized> !Send for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
	fn drop(&mut self) {
		::oro_dbgutil::__oro_dbgutil_lock_release(self.lock.value.get() as *const () as usize);
		self.lock.locked.store(false, Release);
	}
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &*self.lock.value.get() }
	}
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &mut *self.lock.value.get() }
	}
}

/// A ticketed, fair mutex implementation.
pub struct TicketMutex<T: ?Sized> {
	/// The currently served ticket.
	now_serving: AtomicUsize,
	/// The next ticket.
	next_ticket: AtomicUsize,
	/// Whether or not we've locked the lock.
	locked:      AtomicBool,
	/// The guarded value.
	value:       UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for TicketMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for TicketMutex<T> {}

impl<T> TicketMutex<T> {
	/// Creates a new ticket mutex.
	pub const fn new(value: T) -> Self {
		Self {
			now_serving: AtomicUsize::new(0),
			next_ticket: AtomicUsize::new(0),
			locked:      AtomicBool::new(false),
			value:       UnsafeCell::new(value),
		}
	}
}

impl<T: ?Sized> Lock for TicketMutex<T> {
	type Guard<'a>
		= TicketMutexGuard<'a, T>
	where
		T: 'a;
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
					::oro_dbgutil::__oro_dbgutil_lock_acquire(
						self.value.get() as *const () as usize
					);
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

impl<T: Default> Default for TicketMutex<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

/// A lock guard for a [`TicketMutex`].
pub struct TicketMutexGuard<'a, T: ?Sized + 'a>
where
	Self: 'a,
{
	/// Our ticket
	ticket: usize,
	/// The lock we are guarding.
	lock:   &'a TicketMutex<T>,
}

impl<T: ?Sized> Drop for TicketMutexGuard<'_, T> {
	fn drop(&mut self) {
		::oro_dbgutil::__oro_dbgutil_lock_release(self.lock.value.get() as *const () as usize);
		let _ = self.lock.now_serving.compare_exchange(
			self.ticket,
			self.ticket.wrapping_add(1),
			Release,
			Relaxed,
		);
		self.lock.locked.store(false, Release);
	}
}

impl<T: ?Sized> Deref for TicketMutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &*self.lock.value.get() }
	}
}

impl<T: ?Sized> DerefMut for TicketMutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// SAFETY: We have guaranteed singular access as we're locked.
		unsafe { &mut *self.lock.value.get() }
	}
}

impl<T: ?Sized> !Send for TicketMutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for TicketMutexGuard<'_, T> {}

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
		unsafe fn oro_kernel_sync_current_core_id() -> u32;
	}

	/// A reentrant mutex implementation.
	///
	/// This mutex allows the same core to lock the mutex multiple times.
	///
	/// **NOTE:** This implementation spins (and does not lock) if the refcount
	/// reaches `u32::MAX`. This is usually not a problem.
	pub struct ReentrantMutex<T: ?Sized> {
		/// The lock state.
		///
		/// The upper 32 bits are the core ID of the lock holder, and the lower 32 bits
		/// are the lock count.
		lock:  AtomicU64,
		/// The inner value.
		value: UnsafeCell<T>,
	}

	impl<T> ReentrantMutex<T> {
		/// Constructs a new reentrant mutex.
		pub const fn new(inner: T) -> Self {
			Self {
				lock:  AtomicU64::new(0),
				value: UnsafeCell::new(inner),
			}
		}
	}

	impl<T: ?Sized> Lock for ReentrantMutex<T> {
		/// The lock guard type used by the lock implementation.
		type Guard<'a>
			= ReentrantMutexGuard<'a, T>
		where
			T: 'a;
		/// The target type of value being guarded.
		type Target = T;

		fn lock(&self) -> Self::Guard<'_> {
			// SAFETY: The safety requirements for this function are offloaded to the
			// SAFETY: implementation; it's marked unsafe as a requirement by Rust.
			let core_id = unsafe { oro_kernel_sync_current_core_id() };

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
					if current == 0 {
						::oro_dbgutil::__oro_dbgutil_lock_acquire(
							self.value.get() as *const () as usize
						);
					}
					return ReentrantMutexGuard { lock: self };
				}
			}
		}
	}

	impl<T: Default> Default for ReentrantMutex<T> {
		fn default() -> Self {
			Self::new(T::default())
		}
	}

	/// A guard for a reentrant mutex.
	pub struct ReentrantMutexGuard<'a, T: ?Sized + 'a> {
		lock: &'a ReentrantMutex<T>,
	}

	impl<T: ?Sized> core::ops::Deref for ReentrantMutexGuard<'_, T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			// SAFETY: The guard is only created if the lock is held.
			unsafe { &*self.lock.value.get() }
		}
	}

	impl<T: ?Sized> core::ops::DerefMut for ReentrantMutexGuard<'_, T> {
		fn deref_mut(&mut self) -> &mut Self::Target {
			// SAFETY: The guard is only created if the lock is held.
			unsafe { &mut *self.lock.value.get() }
		}
	}

	impl<T: ?Sized> Drop for ReentrantMutexGuard<'_, T> {
		fn drop(&mut self) {
			loop {
				let current = self.lock.lock.load(Relaxed);
				let current_count = current & 0xFFFF_FFFF;

				debug_assert_eq!(
					(current >> 32) as u32,
					unsafe { oro_kernel_sync_current_core_id() },
					"re-entrant lock held lock by another core upon drop"
				);

				if self
					.lock
					.lock
					.compare_exchange(
						current,
						if current_count == 1 { 0 } else { current - 1 },
						Release,
						Relaxed,
					)
					.is_ok()
				{
					if current_count == 1 {
						::oro_dbgutil::__oro_dbgutil_lock_release(
							self.lock.value.get() as *const () as usize,
						);
					}
					break;
				}
			}
		}
	}

	unsafe impl<T: ?Sized + Send> Send for ReentrantMutex<T> {}
	unsafe impl<T: ?Sized + Send> Sync for ReentrantMutex<T> {}
	impl<T: ?Sized> !Send for ReentrantMutexGuard<'_, T> {}
	unsafe impl<T: ?Sized + Sync> Sync for ReentrantMutexGuard<'_, T> {}
}

#[cfg(feature = "reentrant_mutex")]
pub use reentrant::*;
