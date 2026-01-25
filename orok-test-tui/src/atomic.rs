use core::sync::atomic::Ordering::Relaxed;

pub trait RelaxedAtomic<T> {
	fn get(&self) -> T;
	#[expect(unused)]
	fn set(&self, v: T) -> T;
}

macro_rules! impl_relaxed {
	($($ty:ident => $inner:ty),* $(,)?) => {
		$(
			impl RelaxedAtomic<$inner> for core::sync::atomic::$ty {
				#[inline(always)]
				fn get(&self) -> $inner { self.load(Relaxed) }
				#[inline(always)]
				fn set(&self, v: $inner) -> $inner { self.swap(v, Relaxed) }
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

pub trait RelaxedNumericAtomic<T> {
	#[expect(unused)]
	fn increment(&self) -> T;
	#[expect(unused)]
	fn decrement(&self) -> T;
}

macro_rules! impl_relaxed_numeric {
	($($ty:ident => $inner:ty),* $(,)?) => {
		$(
			impl RelaxedNumericAtomic<$inner> for core::sync::atomic::$ty {
				#[inline(always)]
				fn increment(&self) -> $inner { self.fetch_add(1, Relaxed) }
				#[inline(always)]
				fn decrement(&self) -> $inner { self.fetch_sub(1, Relaxed) }
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
