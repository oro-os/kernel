//! Provides a number of compile-time assertion traits that can be used
//! to ensure that certain properties hold for types.
//!
//! Typical usage is to parameterize the traits and bound them to generics,
//! then whenever the assertion should be checked.
//!
//! This will cause a compile-time error if the assertion does not hold.
//!
//! Must not depend on any other crate.
#![cfg_attr(not(test), no_std)]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]

/// Asserts that a type is *within* a number of bytes (i.e. `size_of::<T>() <= SIZE`).
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertFits<const SIZE: usize>: Sized {
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

unsafe impl<T: Sized, const SIZE: usize> AssertFits<SIZE> for T {}

/// One-off assertion that a type fits within a certain size.
pub const fn fits1<Smaller: Sized, const SIZE: usize>(_v: &Smaller) {
	() = <Smaller as AssertFits<SIZE>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise.
pub const fn fits_within<Smaller: Sized, Larger: Sized>() {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise using the smaller's value reference.
pub const fn fits_within1<Smaller: Sized, Larger: Sized>(_v: &Smaller) {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// One-off assertion that a type fits within another type size-wise using value references.
pub const fn fits_within2<Smaller: Sized, Larger: Sized>(_v: &Smaller, _u: &Larger) {
	() = <Smaller as AssertFitsWithin<Larger>>::ASSERT;
}

/// Asserts that a type is exactly a certain size.
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertSizeOf<const SIZE: usize>: Sized {
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

unsafe impl<T: Sized, const SIZE: usize> AssertSizeOf<SIZE> for T {}

/// One-off assertion that a type is a certain size.
pub const fn size_of<T: Sized, const SIZE: usize>() {
	() = <T as AssertSizeOf<SIZE>>::ASSERT;
}

/// One-off assertion that a type is a certain size using value references.
pub const fn size_of1<T: Sized, const SIZE: usize>(_v: &T) {
	() = <T as AssertSizeOf<SIZE>>::ASSERT;
}

/// Asserts that two types have the same size.
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertSizeEq<U: Sized>: Sized {
	/// Performs the assertion that two types have the same size.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <T as AssertSizeEq<U>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		core::mem::size_of::<Self>() == core::mem::size_of::<U>(),
		"types do not have the same size"
	);
}

unsafe impl<T: Sized, U: Sized> AssertSizeEq<U> for T {}

/// One-off assertion that asserts two types have the same size.
pub const fn size_eq<T: Sized, U: Sized>() {
	() = <T as AssertSizeEq<U>>::ASSERT;
}

/// Asserts that a type does not have a destructor (drop method) or have any fields
/// that require a destructor to be called.
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertNoDrop {
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

unsafe impl<T: ?Sized> AssertNoDrop for T {}

/// Asserts that a type does not require a destructor to be called
/// (i.e. does not implement `Drop`, nor has any fields that require `Drop`).
pub const fn no_drop<T: ?Sized>() {
	() = <T as AssertNoDrop>::ASSERT;
}

/// Asserts that a type has equal or less alignment requirements than another type.
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertAlignsWithin<Larger: Sized>: Sized {
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

unsafe impl<Smaller: Sized, Larger: Sized> AssertAlignsWithin<Larger> for Smaller {}

/// One-off assertion that a type has equal or less alignment requirements
/// than a given size.
pub const fn aligns_to<Smaller: Sized, const ALIGN: usize>() {
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
pub const fn aligns_within1<Smaller: Sized, Larger: Sized>(_v: &Smaller) {
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
pub const fn aligns_within2<Smaller: Sized, Larger: Sized>(_v: &Smaller, _u: &Larger) {
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
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
unsafe trait AssertFitsWithin<Larger: Sized>: Sized {
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

unsafe impl<Smaller: Sized, Larger: Sized> AssertFitsWithin<Larger> for Smaller {}

/// Asserts that the two given offsets value are equal.
///
/// **Not meant to be used publicly; it's only publicized for the sake of the
/// `offset_eq!` macro.**
///
/// > **Note:** This trait on its own is rather useless; it's just an equality
/// > check. However its error message is specific to offsets, so its usage
/// > in other scenarios is discouraged.
///
/// # Safety
/// The assertion **does not trigger** unless the above explicit usage of the
/// `ASSERT` associated constant is used. There's, unfortunately, no great way
/// to enforce this at the type level.
#[doc(hidden)]
pub unsafe trait AssertOffsetEq<const LHS: usize, const RHS: usize> {
	/// Performs the assertion that the two offsets are equal.
	///
	/// This must be referenced somewhere in the code at each usage site,
	/// like so:
	///
	/// ```rust
	/// () = <T as AssertOffsetEq<LHS, RHS>>::ASSERT;
	/// ```
	///
	/// This will cause a compile-time error if the assertion does not hold.
	const ASSERT: () = assert!(
		LHS == RHS,
		"offsets are not equal (check `$T` and `$field_name`)"
	);
}

unsafe impl<const LHS: usize, const RHS: usize> AssertOffsetEq<LHS, RHS> for () {}

/// Asserts that the offset of the given field is equal to the specified value.
///
/// **The field must be visible to the callsite.**
///
/// Can be used exactly like the [`core::mem::offset_of!`] macro, but with
/// only a single field.
#[macro_export]
macro_rules! offset_of {
	($T:ty, $field_name:ident, $offset:expr) => {{
		const _: () = <() as $crate::AssertOffsetEq<
			$offset,
			{ ::core::mem::offset_of!($T, $field_name) },
		>>::ASSERT;
	}};
}
