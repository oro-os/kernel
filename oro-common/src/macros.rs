//! Common macros used throughout the codebase.
//!
//! All macros in this module are considered "safe",
//! as in they do not need to be used in an `unsafe`
//! context.
//!
//! Note that "safe" does not mean "stable" - some macros
//! here are only functional in unstable Rust.

/// Stub for [`core::intrinsics::likely`].
#[macro_export]
macro_rules! likely {
	($e:expr) => {{ ::core::intrinsics::likely($e) }};
}

/// Stub for [`core::intrinsics::unlikely`].
#[macro_export]
macro_rules! unlikely {
	($e:expr) => {{ ::core::intrinsics::unlikely($e) }};
}
