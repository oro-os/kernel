//! Common macros used throughout the codebase.
//!
//! All macros in this module are considered "safe",
//! as in they do not need to be used in an `unsafe`
//! context.
//!
//! Note that "safe" does not mean "stable" - some macros
//! here are only functional in unstable Rust.

/// Nightly stub for [`core::intrinsics::likely`].
/// If the `unstable` feature is not enabled, this
/// macro does nothing.
#[macro_export]
macro_rules! likely {
	($e:expr) => {{
		#[cfg(feature = "unstable")]
		{
			::core::intrinsics::likely($e)
		}

		#[cfg(not(feature = "unstable"))]
		{
			$e
		}
	}};
}

/// Nightly stub for [`core::intrinsics::unlikely`].
/// If the `unstable` feature is not enabled, this
/// macro does nothing.
#[macro_export]
macro_rules! unlikely {
	($e:expr) => {{
		#[cfg(feature = "unstable")]
		{
			::core::intrinsics::unlikely($e)
		}

		#[cfg(not(feature = "unstable"))]
		{
			$e
		}
	}};
}
