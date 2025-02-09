//! Basic spin locks for use by the Oro crate.

// NOTE(qix-): DO NOT PUBLICIZE THIS MODULE.
// NOTE(qix-): This module is for internal use only.

use core::{
	cell::UnsafeCell,
	sync::atomic::{
		AtomicBool,
		Ordering::{AcqRel, Acquire, Release},
	},
};

/// A simple spin lock mutex.
pub struct Mutex<T: ?Sized> {
	/// The lock state.
	locked: AtomicBool,
	/// The data protected by the lock.
	data:   UnsafeCell<T>,
}

impl<T> Mutex<T> {
	/// Creates a new `Mutex` instance.
	#[must_use]
	pub const fn new(data: T) -> Self {
		Self {
			locked: AtomicBool::new(false),
			data:   UnsafeCell::new(data),
		}
	}

	/// Attempts to lock the mutex, returning `None` if it's already locked.
	#[must_use]
	pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
		if self.locked.swap(true, AcqRel) {
			None
		} else {
			Some(MutexGuard { mutex: self })
		}
	}

	/// Locks the mutex, spinning until it's available.
	#[must_use]
	pub fn lock(&self) -> MutexGuard<'_, T> {
		loop {
			if let Some(guard) = self.try_lock() {
				return guard;
			}
		}
	}
}

impl<T: ?Sized> Drop for Mutex<T> {
	fn drop(&mut self) {
		debug_assert!(!self.locked.load(Acquire), "Mutex dropped while locked");
	}
}

/// A guard for a locked [`Mutex`].
pub struct MutexGuard<'a, T: ?Sized> {
	/// The mutex being guarded.
	mutex: &'a Mutex<T>,
}

impl<T: ?Sized> core::ops::Deref for MutexGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.mutex.data.get() }
	}
}

impl<T: ?Sized> core::ops::DerefMut for MutexGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.mutex.data.get() }
	}
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
	fn drop(&mut self) {
		debug_assert!(
			self.mutex.locked.load(Acquire),
			"MutexGuard dropped without lock"
		);
		self.mutex.locked.store(false, Release);
	}
}
