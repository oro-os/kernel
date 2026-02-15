//! Relaxed types for atomic operations.

#[doc(hidden)]
macro_rules! impl_relaxed {
	($ident:ident, $atomic:ident, $inner:ty) => {
		#[doc = concat!("A relaxed atomic ", stringify!($inner), ".")]
		#[derive(Debug)]
		#[repr(transparent)]
		pub struct $ident(core::sync::atomic::$atomic);

		impl $ident {
			/// Creates a new relaxed atomic of the given type.
			#[inline(always)]
			#[must_use]
			pub const fn new(value: $inner) -> Self {
				Self(<core::sync::atomic::$atomic>::new(value))
			}

			/// Loads the value of the relaxed atomic.
			#[inline(always)]
			#[must_use]
			pub fn load(&self) -> $inner {
				self.0.load(core::sync::atomic::Ordering::Relaxed)
			}

			/// Stores a value into the relaxed atomic.
			#[inline(always)]
			pub fn store(&self, value: $inner) {
				self.0.store(value, core::sync::atomic::Ordering::Relaxed);
			}

			/// Swaps the value of the relaxed atomic with a new value, returning the old value.
			#[inline(always)]
			#[must_use]
			pub fn swap(&self, value: $inner) -> $inner {
				self.0.swap(value, core::sync::atomic::Ordering::Relaxed)
			}
		}
	};
}

impl_relaxed!(RelaxedBool, AtomicBool, bool);

impl_relaxed!(RelaxedUsize, AtomicUsize, usize);
impl_relaxed!(RelaxedU64, AtomicU64, u64);
impl_relaxed!(RelaxedU32, AtomicU32, u32);
impl_relaxed!(RelaxedU16, AtomicU16, u16);
impl_relaxed!(RelaxedU8, AtomicU8, u8);

impl_relaxed!(RelaxedIsize, AtomicIsize, isize);
impl_relaxed!(RelaxedI64, AtomicI64, i64);
impl_relaxed!(RelaxedI32, AtomicI32, i32);
impl_relaxed!(RelaxedI16, AtomicI16, i16);
impl_relaxed!(RelaxedI8, AtomicI8, i8);
