//! A set of macros useful for working with unsafe code.

/// Utility macro that requires that it's present inside of an
/// unsafe block. Useful for other macros that must only be
/// used in an unsafe context.
#[macro_export]
macro_rules! assert_unsafe {
	() => {{
		#[allow(clippy::zero_ptr)]
		let _ptr = 0 as *const ();
		let _this_macro_must_be_used_in_an_unsafe_context = *_ptr;
	}};
}

/// A non-I/O macro that will halt the CPU in the event
/// a precondition is not met. Only enabled in debug mode.
#[macro_export]
macro_rules! unsafe_precondition {
	($Target:ty, $cond:expr, $_note:literal) => {
		$crate::assert_unsafe!();

		#[cfg(debug_assertions)]
		if $crate::unlikely!(!$cond) {
			<$Target as $crate::arch::Arch>::halt();
		}

		#[cfg(not(debug_assertions))]
		if 1 == 0 {
			// This is a no-op in release mode.
			// We do this so that $A is always used and
			// doesn't get a compiler error when it's coming
			// from a template parameter.
			<$Target as $crate::arch::Arch>::halt();
		}
	};

	// NOTE: requires oro_arch as a dependency
	($cond:expr, $_note:literal) => {
		unsafe_precondition!(::oro_arch::Target, $cond, $_note);
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

/// Performs a critical section, disabling interrupts for the
/// duration of the block.
///
/// # Safety
/// The block **MUST NOT** panic under ANY circumstances.
#[macro_export]
macro_rules! critical_section {
	($Arch:ty, $body:block) => {{
		$crate::assert_unsafe!();

		let state = <$Arch as $crate::Arch>::fetch_interrupts();
		<$Arch as $crate::Arch>::disable_interrupts();
		let result = { $body };
		<$Arch as $crate::Arch>::restore_interrupts(state);
		result
	}};
}
