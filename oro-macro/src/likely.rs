//! Likely/unlikely macros for branch prediction hints.

// XXX TODO(qix-): **(UN)LIKELY IS CURRENTLY BUGGED**.
// XXX TODO(qix-): For now, it's been disabled and will
// XXX TODO(qix-): simply pass through the expression
// XXX TODO(qix-): until it's fixed. Tracking issue:
// XXX TODO(qix-): https://github.com/rust-lang/rust/issues/88767

/// Stub for [`core::intrinsics::likely`].
#[macro_export]
macro_rules! likely {
	($e:expr) => {{ $e }}; // ($e:expr) => {{ ::core::intrinsics::likely($e) }};
}

/// Stub for [`core::intrinsics::unlikely`].
#[macro_export]
macro_rules! unlikely {
	($e:expr) => {{ $e }}; // ($e:expr) => {{ ::core::intrinsics::unlikely($e) }};
}
