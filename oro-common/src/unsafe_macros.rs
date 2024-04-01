//! A set of macros useful for working with unsafe code.

/// Utility macro that requires that it's present inside of an
/// unsafe block. Useful for other macros that must only be
/// used in an unsafe context.
#[macro_export]
macro_rules! assert_unsafe {
	() => {{
		let _ptr = 0 as *const ();
		let _this_macro_must_be_used_in_an_unsafe_context = *_ptr;
	}};
}

/// A non-I/O macro that will halt the CPU in the event
/// a precondition is not met. Only enabled in debug mode.
#[macro_export]
macro_rules! unsafe_precondition {
	($Arch:ty, $cond:expr, $_note:literal) => {
		$crate::assert_unsafe!();

		#[cfg(debug_assertions)]
		if !$cond {
			<$Arch as $crate::Arch>::halt();
		}

		#[cfg(not(debug_assertions))]
		if 1 == 0 {
			// This is a no-op in release mode.
			// We do this so that $A is always used and
			// doesn't get a compiler error when it's coming
			// from a template parameter.
			<$Arch as $crate::Arch>::halt();
		}
	};
}

/// Workaround for a `#[non-exhaustive]` enum with a `#[repr([uN)]`
/// representation that might have bit representations not explicitly
/// listed as variants.
///
/// This would normally be undefined, but assuming the `as $ty`
/// syntax (see below) matches the representation of the enum, this
/// is a well defined workaround to catch the undefined values
/// as a default branch.
///
/// # Safety
/// The `$ty` must be the same type as the enum's representation.
#[macro_export]
macro_rules! match_nonexhaustive {
	(match $match:expr => $ty:ty {
		$($pat:expr => $expr:expr),+
		, % $default_name:ident => $default_expr:expr $(,)?
	}) => {{
		$crate::assert_unsafe!();

		match ::core::mem::transmute::<Self, $ty>($match) {
			$(v if v == $pat as $ty => $expr),+
			, $default_name => $default_expr
		}
	}};
}
