//! Synchronization primitives and access utilities
//! for use in shared resource scenarios.

use crate::arch::run_critical_section;
use ::core::ops::{Deref, DerefMut, FnOnce};
use ::spin::{
	mutex::{SpinMutex, SpinMutexGuard, TicketMutex, TicketMutexGuard},
	RwLock,
};

/// An exclusive lock (only one owner at a time)
/// where pending ownership requests are granted
/// in no guaranteed order (unfair)
///
/// Mutated with [`map_mut`].
pub type UnfairMutex<T> = SpinMutex<T>;
/// Guard type for [`UnfairMutex`]
pub type UnfairMutexGuard<'a, A> = SpinMutexGuard<'a, A>;
/// An exclusive lock (only one owner at a time)
/// where pending ownership requests are granted
/// in order of request (fair). Also known as
/// a "ticket" mutex or FIFO lock.
///
/// Mutated with [`map_mut`].
pub type FairMutex<T> = TicketMutex<T>;
/// Guard type for [`FairMutex`]
pub type FairMutexGuard<'a, A> = TicketMutexGuard<'a, A>;
/// A read-write lock, whereby multiple readers
/// may obtain immutable grants to use the underlying
/// data at once, but only one writer may obtain
/// a mutable grant to mutate the underlying data,
/// and where pending ownership requests are granted
/// in no guaranteed order (unfair)
///
/// Read locks are acquired via [`map_read`]. Write
/// locks are obtained via [`map_write`].
pub type UnfairRwMutex<T> = RwLock<T>;

#[doc(hidden)]
pub trait Lockable<'a, A: ?Sized> {
	type Guard: DerefMut<Target = A>;
	fn lock(self) -> Self::Guard;
}

impl<'a, A> Lockable<'a, A> for &'a UnfairMutex<A> {
	type Guard = UnfairMutexGuard<'a, A>;

	#[doc(hidden)]
	fn lock(self) -> Self::Guard {
		(self as &'a UnfairMutex<A>).lock()
	}
}

impl<'a, A> Lockable<'a, A> for &'a FairMutex<A> {
	type Guard = FairMutexGuard<'a, A>;

	fn lock(self) -> Self::Guard {
		(self as &'a FairMutex<A>).lock()
	}
}

/// Obtain an exclusive, uninterrupted lock on a mutex and
/// run a functor on a mutable reference to its underlying data
#[allow(unused)]
pub fn map_mut<'a, T: ?Sized, F, R, Mutex>(mutex: Mutex, func: F) -> R
where
	Mutex: Lockable<'a, T>,
	F: FnOnce(&mut T) -> R,
{
	run_critical_section(|| {
		let mut guard = mutex.lock();
		func(guard.deref_mut())
	})
}

/// Obtain a shared, uninterrupted read-only lock on a read-write
/// lock and run a functor on an immutable reference to its underlying data
#[allow(unused)]
pub fn map_read<T: ?Sized, F, R>(mutex: &UnfairRwMutex<T>, func: F) -> R
where
	F: FnOnce(&T) -> R,
{
	run_critical_section(|| {
		let guard = mutex.read();
		func(guard.deref())
	})
}

/// Obtain an exclusive, uninterrupted write lock on a read-write
/// lock and run a functor on an immutable reference to its underlying data
#[allow(unused)]
pub fn map_write<T: ?Sized, F, R>(mutex: &UnfairRwMutex<T>, func: F) -> R
where
	F: FnOnce(&mut T) -> R,
{
	run_critical_section(|| {
		let mut guard = mutex.write();
		func(guard.deref_mut())
	})
}
