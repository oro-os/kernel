//! Safe and efficient abstraction for a table of values indexed by a unique ID.

// TODO(qix-): Refactor this to use a home-grown refactor of `hashbrown`'s `HashMap`
// TODO(qix-): to not need the hasher at all. This is a bit of a hack.

use core::{
	alloc::Allocator,
	hash::{BuildHasher, Hasher},
};

use hashbrown::HashMap;
use oro_mem::alloc::alloc::Global;

/// A table of values indexed by a unique ID.
///
/// Allows the insertion of values with an automatically guaranteed system-wide unique ID.
#[repr(transparent)]
pub struct Table<T: Sized, Alloc: Allocator + Default = Global>(
	HashMap<u64, T, StrictIdentityBuildHasher, Alloc>,
);

impl<T: Sized, Alloc: Allocator + Default> Table<T, Alloc> {
	/// Creates a new empty table.
	#[must_use]
	pub fn new() -> Self {
		Self(HashMap::default())
	}

	/// Inserts a value into the table with a specific ID.
	///
	/// Safely checks for collisions and drops the old value if one exists.
	#[inline]
	pub fn insert(&mut self, id: u64, value: T) {
		self.0.insert(id, value);
	}

	/// Inserts a value into the table with a specific ID without checking for
	/// collisions.
	///
	/// # Safety
	/// Caller must ensure that `id` does not already exist in the table.
	#[inline]
	pub unsafe fn insert_unique_unchecked(&mut self, id: u64, value: T) {
		self.0.insert_unique_unchecked(id, value);
	}

	/// Returns a reference to the value associated with the given ID.
	///
	/// Returns `None` if the ID does not exist in the table.
	#[inline]
	pub fn get(&self, id: u64) -> Option<&T> {
		self.0.get(&id)
	}

	/// Removes a value given its key. Returns `None` if the value didn't exist.
	#[inline]
	pub fn remove(&mut self, id: u64) -> Option<T> {
		self.0.remove(&id)
	}
}

/// Builder for a [`StrictIdentityHasher`].
#[derive(Clone, Copy, Debug, Default)]
struct StrictIdentityBuildHasher;

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
struct StrictIdentityHasher {
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
			debug_assert_ne!(
				self.value, 0,
				"StrictIdentityHasher::finish called, but hash is 0 (unique IDs cannot be 0)"
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
