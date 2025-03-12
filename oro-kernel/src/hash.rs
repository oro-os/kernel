//! Hashing implementations.
//!
//! Note that these implementations may impose strict rules for
//! how they are used, and under which circumstances they are safe.

use core::hash::{BuildHasher, Hasher};

/// Builder for a [`StrictIdentityHasher`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StrictIdentityBuildHasher;

impl BuildHasher for StrictIdentityBuildHasher {
	type Hasher = StrictIdentityHasher;

	fn build_hasher(&self) -> Self::Hasher {
		StrictIdentityHasher::default()
	}
}

/// A strict identity hasher that only hashes `u64` values (by
/// returning the value itself). Only allows a single `u64`
/// to be passed to the hasher.
///
/// This is drastically unsafe in most cases, but allows a very
/// efficient hash implementation for the `Table` type, whereby
/// we can guarantee that only `u64` values are hashed and whereby
/// they are unique.
///
/// # Safety
/// This hasher is only safe to use in the context of the `Table`
/// type. **Do not use it for any other purpose.** In debug builds,
/// it will panic with an assertion if it is used incorrectly.
///
/// In release builds, it becomes must less safe if used with
/// multiple calls to `write_u64`. It also uses `unreachable!`
/// for other hash types in release builds, as opposed to a
/// `debug_assert!` in debug builds.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrictIdentityHasher {
	/// The hashed value, or 0 if it's not been hashed.
	value: u64,
	/// Debug-only: whether the hasher has populated a value.
	#[cfg(debug_assertions)]
	used:  bool,
}

impl Hasher for StrictIdentityHasher {
	fn write(&mut self, _bytes: &[u8]) {
		debug_assert!(false, "StrictIdentityHasher::write called");
		unreachable!();
	}

	fn finish(&self) -> u64 {
		// NOTE(qix-): We have to use the attribute since `debug_assert!` is implemented
		// NOTE(qix-): as an `if cfg!(...)` block, which means the expressions must refer
		// NOTE(qix-): to 'real' identifiers even in debug mode. It makes sense, but gets
		// NOTE(qix-): in the way here.
		#[cfg(debug_assertions)]
		{
			debug_assert!(
				self.used,
				"StrictIdentityHasher::finish called before any writes"
			);
		}

		self.value
	}

	fn write_u64(&mut self, i: u64) {
		#[cfg(debug_assertions)]
		{
			debug_assert!(
				!self.used,
				"StrictIdentityHasher::write_u64 called multiple times"
			);

			self.used = true;
		}

		self.value = i;
	}

	fn write_i128(&mut self, _i: i128) {
		debug_assert!(false, "StrictIdentityHasher::write_i128 called");
		unreachable!();
	}

	fn write_i16(&mut self, _i: i16) {
		debug_assert!(false, "StrictIdentityHasher::write_i16 called");
		unreachable!();
	}

	fn write_i32(&mut self, _i: i32) {
		debug_assert!(false, "StrictIdentityHasher::write_i32 called");
		unreachable!();
	}

	fn write_i64(&mut self, _i: i64) {
		debug_assert!(false, "StrictIdentityHasher::write_i64 called");
		unreachable!();
	}

	fn write_i8(&mut self, _i: i8) {
		debug_assert!(false, "StrictIdentityHasher::write_i8 called");
		unreachable!();
	}

	fn write_isize(&mut self, _i: isize) {
		debug_assert!(false, "StrictIdentityHasher::write_isize called");
		unreachable!();
	}

	fn write_u128(&mut self, _i: u128) {
		debug_assert!(false, "StrictIdentityHasher::write_u128 called");
		unreachable!();
	}

	fn write_u16(&mut self, _i: u16) {
		debug_assert!(false, "StrictIdentityHasher::write_u16 called");
		unreachable!();
	}

	fn write_u32(&mut self, _i: u32) {
		debug_assert!(false, "StrictIdentityHasher::write_u32 called");
		unreachable!();
	}

	fn write_u8(&mut self, _i: u8) {
		debug_assert!(false, "StrictIdentityHasher::write_u8 called");
		unreachable!();
	}

	fn write_usize(&mut self, _i: usize) {
		debug_assert!(false, "StrictIdentityHasher::write_usize called");
		unreachable!();
	}
}
