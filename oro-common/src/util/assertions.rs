//! Provides a number of compile-time assertion traits that can be used
//! to ensure that certain properties hold for types.
//!
//! Typical usage is to parameterize the traits and bound them to generics,
//! then whenever the assertion should be checked, to use something like this:
//!
//! ```rust
//! () = <T as SomeAssertion>::ASSERT;
//! ```
//!
//! This will cause a compile-time error if the assertion does not hold.
//!
//! # Safety
//! The assertion **does not trigger** unless the above explicit usage of the
//! `ASSERT` associated constant is used. There's, unfortunately, no great way
//! to enforce this at the type level.

/// Asserts that a type is *within* a number of bytes (i.e. `size_of::<T>() <= SIZE`).
///
/// To use, simply bound a type to this trait and use the `ASSERT` associated constant
/// like so:
///
/// ```rust
/// () = <T as AssertFits<SIZE>>::ASSERT;
/// ```
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
pub trait AssertFits<const SIZE: usize>: Sized {
	/// Performs the assertion that the type fits within the specified size.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <T as AssertFits<SIZE>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		core::mem::size_of::<Self>() <= SIZE,
		"value does not fit into the specified size (check SIZE)"
	);
}

impl<T: Sized, const SIZE: usize> AssertFits<SIZE> for T {}

/// One-off assertion that a type fits within a certain size.
pub fn assert_fits<T: Sized, const SIZE: usize>(_v: &T) {
	() = <T as AssertFits<SIZE>>::ASSERT;
}

/// Asserts that a type does not have a destructor (drop method) or have any fields
/// that require a destructor to be called.
///
/// To use, simply bound a type to this trait and use the `ASSERT` associated constant
/// like so:
///
/// ```rust
/// () = <T as AssertNoDrop>::ASSERT;
/// ```
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
pub trait AssertNoDrop {
	/// Performs the assertion that the type does not have a destructor.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <T as AssertNoDrop>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		!core::mem::needs_drop::<Self>(),
		"the value must not have a destructor (drop method) or have any fields that require a \
		 destructor to be called"
	);
}

impl<T> AssertNoDrop for T {}
