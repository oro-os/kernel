use crate::arch::run_critical_section;
use ::core::ops::{DerefMut, FnOnce};
pub use ::spin::mutex::{
	SpinMutex as UnfairMutex, SpinMutexGuard as UnfairMutexGuard, TicketMutex as FairMutex,
	TicketMutexGuard as FairMutexGuard,
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

pub fn map_sync_mut<'a, T: ?Sized, F, R, Mutex>(mutex: Mutex, func: F) -> R
where
	Mutex: Lockable<'a, T>,
	F: FnOnce(&mut T) -> R,
{
	run_critical_section(|| {
		let mut guard = mutex.lock();
		func(guard.deref_mut())
	})
}
