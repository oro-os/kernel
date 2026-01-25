//! Relaxed types for atomic operations.

use core::sync::atomic::{
	AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed,
};

#[doc(hidden)]
macro_rules! impl_relaxed {
	($ident:ident, $atomic:ty, $inner:ty) => {
		#[doc = concat!("A relaxed atomic ", stringify!($inner), ".")]
		#[derive(Debug)]
		#[repr(transparent)]
		pub struct $ident($atomic);

		impl $ident {
			/// Creates a new relaxed atomic of the given type.
			#[inline(always)]
			#[must_use]
			pub const fn new(value: $inner) -> Self {
				Self(<$atomic>::new(value))
			}

			/// Loads the value of the relaxed atomic.
			#[inline(always)]
			#[must_use]
			pub fn load(&self) -> $inner {
				self.0.load(Relaxed)
			}

			/// Stores a value into the relaxed atomic.
			#[inline(always)]
			pub fn store(&self, value: $inner) {
				self.0.store(value, Relaxed);
			}

			/// Swaps the value of the relaxed atomic with a new value, returning the old value.
			#[inline(always)]
			#[must_use]
			pub fn swap(&self, value: $inner) -> $inner {
				self.0.swap(value, Relaxed)
			}
		}
	};
}

impl_relaxed!(RelaxedBool, AtomicBool, bool);
impl_relaxed!(RelaxedUsize, AtomicUsize, usize);
impl_relaxed!(RelaxedU64, AtomicU64, u64);
impl_relaxed!(RelaxedU32, AtomicU32, u32);
impl_relaxed!(RelaxedU16, AtomicU16, u16);
