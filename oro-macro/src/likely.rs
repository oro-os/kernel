//! Likely/unlikely macros for branch prediction hints.

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
