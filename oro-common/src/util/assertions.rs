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
#![allow(dead_code)]

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
pub const fn assert_fits1<Smaller: Sized, const SIZE: usize>(_v: &Smaller) {
	() = <Smaller as AssertFits<SIZE>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise.
pub const fn assert_fits_within<Smaller: Sized, Larger: Sized>() {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise using the smaller's value reference.
pub const fn assert_fits_within1<Smaller: Sized, Larger: Sized>(_v: &Smaller) {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise using value references.
pub const fn assert_fits_within2<Smaller: Sized, Larger: Sized>(_v: &Smaller, _u: &Larger) {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// Asserts that a type is exactly a certain size.
///
/// To use, simply bound a type to this trait and use the `ASSERT` associated constant
/// like so:
///
/// ```rust
/// () = <T as AssertSizeOf<SIZE>>::ASSERT;
/// ```
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
pub trait AssertSizeOf<const SIZE: usize>: Sized {
	/// Performs the assertion that the type is exactly the specified size.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <T as AssertSizeOf<SIZE>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		core::mem::size_of::<Self>() == SIZE,
		"value is not the specified size (check SIZE)"
	);
}

impl<T: Sized, const SIZE: usize> AssertSizeOf<SIZE> for T {}

/// One-off assertion that a type is a certain size.
pub const fn assert_size_of<T: Sized, const SIZE: usize>() {
	() = <T as AssertSizeOf<SIZE>>::ASSERT;
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

/// Asserts that a type has equal or less alignment requirements than another type.
///
/// To use, simply bound a type to this trait and use the `ASSERT` associated constant
/// like so:
///
/// ```rust
/// () = <Smaller as AssertAlignsWithin<Larger>>::ASSERT;
/// ```
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
pub trait AssertAlignsWithin<Larger: Sized>: Sized {
	/// Performs the assertion that the type has equal or less alignment requirements
	/// than another type.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <Smaller as AssertAlignsWithin<Larger>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		core::mem::align_of::<Self>() <= core::mem::align_of::<Larger>(),
		"value does not align within the specified type (check Larger type)"
	);
}

impl<Smaller: Sized, Larger: Sized> AssertAlignsWithin<Larger> for Smaller {}

/// One-off assertion that a type has equal or less alignment requirements
/// than a given size.
pub const fn assert_aligns_to<Smaller: Sized, const ALIGN: usize>() {
	() = assert!(ALIGN.is_power_of_two(), "ALIGN must be a power of two");
	// This is a sanity check; it should always be true.
	// If it's not, a language-level guarantee has been violated.
	() = assert!(
		core::mem::align_of::<Smaller>().is_power_of_two(),
		"(sanity check) Smaller type has non-power-of-two alignment!"
	);
	() = assert!(
		core::mem::align_of::<Smaller>() <= ALIGN,
		"value does not align to the specified size (check ALIGN)"
	);
}

/// One-off assertion that a type has equal or less alignment requirements
/// than another type.
pub const fn assert_aligns_within1<Smaller: Sized, Larger: Sized>(_v: &Smaller) {
	// These are sanity checks; they should always be true.
	// If they're not, a language-level guarantee has been violated.
	() = assert!(
		core::mem::align_of::<Smaller>().is_power_of_two(),
		"(sanity check) Smaller type has non-power-of-two alignment!"
	);
	() = assert!(
		core::mem::align_of::<Larger>().is_power_of_two(),
		"(sanity check) Larger type has non-power-of-two alignment!"
	);
	() = <Smaller as AssertAlignsWithin<Larger>>::ASSERT;
}

/// One-off assertion that a type has equal or less alignment requirements
/// than another type using value references.
pub const fn assert_aligns_within2<Smaller: Sized, Larger: Sized>(_v: &Smaller, _u: &Larger) {
	// These are sanity checks; they should always be true.
	// If they're not, a language-level guarantee has been violated.
	() = assert!(
		core::mem::align_of::<Smaller>().is_power_of_two(),
		"(sanity check) Smaller type has non-power-of-two alignment!"
	);
	() = assert!(
		core::mem::align_of::<Larger>().is_power_of_two(),
		"(sanity check) Larger type has non-power-of-two alignment!"
	);
	() = <Smaller as AssertAlignsWithin<Larger>>::ASSERT;
}

/// Asserts that a type fits within another type size-wise.
///
/// To use, simply bound a type to this trait and use the `ASSERT` associated constant
/// like so:
///
/// ```rust
/// () = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
/// ```
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
pub trait AssertFitsWithin<Larger: Sized>: Sized {
	/// Performs the assertion that the type fits within another type size-wise.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		core::mem::size_of::<Self>() <= core::mem::size_of::<Larger>(),
		"value does not fit within the specified type (check Larger type)"
	);
}

impl<Smaller: Sized, Larger: Sized> AssertFitsWithin<Larger> for Smaller {}
