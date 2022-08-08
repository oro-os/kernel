use crate::arch::run_critical_section;
use ::core::ops::{Deref, DerefMut, FnOnce};
pub use ::spin::{
	mutex::{
		SpinMutex as UnfairMutex, SpinMutexGuard as UnfairMutexGuard, TicketMutex as FairMutex,
		TicketMutexGuard as FairMutexGuard,
	},
	RwLock as UnfairRwMutex,
};

pub trait Lockable<'a, A: ?Sized> {
	type Guard: DerefMut<Target = A>;
	fn lock(self) -> Self::Guard;
}

impl<'a, A> Lockable<'a, A> for &'a UnfairMutex<A> {
	type Guard = UnfairMutexGuard<'a, A>;

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
