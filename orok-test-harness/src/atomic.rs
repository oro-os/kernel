//! Atomic extension trait for relaxed ordering operations.
//!
//! Purely internal convenience trait to DRY up the constraint
//! checking code in `State`. Not intended for general use.

use core::sync::atomic::Ordering::Relaxed;

/// Extension trait for relaxed atomics, which are used for all state tracking in the test harness.
pub trait RelaxedAtomic<T> {
	/// Return the current value of the atomic, using relaxed ordering.
	fn get(&self) -> T;
	/// Set (swap) the value of the atomic, using relaxed ordering, and return the previous value.
	fn set(&self, value: T) -> T;
}

#[doc(hidden)]
macro_rules! impl_relaxed {
	($($ty:ident => $inner:ty),* $(,)?) => {
		$(
			impl RelaxedAtomic<$inner> for core::sync::atomic::$ty {
				#[inline(always)]
				fn get(&self) -> $inner { self.load(Relaxed) }
				#[inline(always)]
				fn set(&self, value: $inner) -> $inner { self.swap(value, Relaxed) }
			}
		)*
	}
}

impl_relaxed! {
	AtomicBool => bool,
	AtomicUsize => usize,
	AtomicU64 => u64,
	AtomicU32 => u32,
	AtomicU16 => u16,
	AtomicU8 => u8,
	AtomicIsize => isize,
	AtomicI64 => i64,
	AtomicI32 => i32,
	AtomicI16 => i16,
	AtomicI8 => i8,
}

/// Extension trait for relaxed atomics that are numeric-like.
pub trait RelaxedNumericAtomic<T> {
	/// Increment the value of the atomic by 1, using relaxed ordering, and return the previous value.
	fn increment(&self) -> T;
}

#[doc(hidden)]
macro_rules! impl_relaxed_numeric {
	($($ty:ident => $inner:ty),* $(,)?) => {
		$(
			impl RelaxedNumericAtomic<$inner> for core::sync::atomic::$ty {
				#[inline(always)]
				fn increment(&self) -> $inner { self.fetch_add(1, Relaxed) }
			}
		)*
	}
}

impl_relaxed_numeric! {
	AtomicUsize => usize,
	AtomicU64 => u64,
	AtomicU32 => u32,
	AtomicU16 => u16,
	AtomicU8 => u8,
	AtomicIsize => isize,
	AtomicI64 => i64,
	AtomicI32 => i32,
	AtomicI16 => i16,
	AtomicI8 => i8,
}
