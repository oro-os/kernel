//! A set of macros useful for working with unsafe code.

/// Utility macro that requires that it's present inside of an
/// unsafe block. Useful for other macros that must only be
/// used in an unsafe context.
#[macro_export]
macro_rules! assert_unsafe {
	() => {{
		const {
			const unsafe fn macro_requires_unsafe() {}
			macro_requires_unsafe();
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
