//! Provides debug output macros. These should be used over
//! calling into the [`crate::arch::Arch`] functions directly when
//! logging output.
#![allow(unused_macros, clippy::module_name_repetitions)]

/// Sends a general debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg {
	($Target:ty, $tag:literal, $($arg:tt)*) => {{
		<$Target as $crate::arch::Arch>::log(format_args!(" :{}:{}", $tag, format_args!($($arg)*)));
	}};

	($tag:literal, $($arg:tt)*) => {{
		// NOTE: must have `oro_arch` in dependencies
		dbg!(::oro_arch::Target, $tag, $($arg)*);
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg_err {
	($Target:ty, $tag:literal, $($arg:tt)*) => {{
		<$Target as $crate::arch::Arch>::log(format_args!("E:{}:{}", $tag, format_args!($($arg)*)));
	}};

	($tag:literal, $($arg:tt)*) => {{
		// NOTE: must have `oro_arch` in dependencies
		dbg_err!(::oro_arch::Target, $tag, $($arg)*);
	}};
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! dbg_warn {
	($Target:ty, $tag:literal, $($arg:tt)*) => {{
		<$Target as $crate::arch::Arch>::log(format_args!("W:{}:{}", $tag, format_args!($($arg)*)));
	}};

	($tag:literal, $($arg:tt)*) => {{
		// NOTE: must have `oro_arch` in dependencies
		dbg_warn!(::oro_arch::Target, $tag, $($arg)*);
	}};
}
