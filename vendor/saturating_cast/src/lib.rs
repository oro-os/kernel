//! Saturating casts for integer primitives.
//!
//! ## Features
//!
//! - `no_std` by default
//! - Saturating casts implemented between all of the following:
//!   - `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
//!   - `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
//! - Saturating traits can be implemented for user types
//!
//! ## Description
//!
//! A saturating cast is a combined [`clamp`][clamp] and cast from one type to
//! another.
//!
//! The cast is effectively `value.clamp(T::MIN, T::MAX) as T`, returning the
//! minimum if `value` is less than the target minimum or the maximum if greater
//! than the target maximum.
//!
//! [clamp]: https://doc.rust-lang.org/stable/std/cmp/trait.Ord.html#method.clamp
//!
//! ## Examples
//!
//! In the following code, the `i32` value is clamped to `255` since `u8` has a
//! range of `0..=255` and `1024` is larger than the maximum value that `u8` can
//! represent.
//!
//! In the second snippet, an `i8` with negative sign is clamped to `0` when
//! cast to `u16` because unsigned integers cannot be negative.
//!
//! ```
//! use saturating_cast::SaturatingCast;
//!
//! let x: i32 = 1024;
//! let y: u8 = 10;
//! let z = x.saturating_cast::<u8>() - y;
//! assert_eq!(245, z);
//!
//! let a = -128_i8;
//! let b: u16 = a.saturating_cast();
//! assert_eq!(0, b);
//! ```
//!
//! Saturating casts **do not** preserve the original value if the source type
//! value is out of range for the target type: the resulting value will be the
//! minimum or maximum of the target type.
//!
//! <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
//! <strong>Note:</strong> This crate only casts and returns primitive integers.
//! Saturating arithmetic is still required to avoid wraparound in release mode
//! and panicking in debug mode (or with overflow checks on in release mode).
//! </p>
//!
//! ## Implementing saturating casts for custom types
//!
//! The following code implements the two traits needed for saturating casts
//! from `Int` to `Uint`. The functionality for saturation is defined within
//! [`SaturatingElement`]'s `as_element`.
//!
//! ```
//! use saturating_cast::{SaturatingCast, SaturatingElement};
//!
//! #[derive(Clone, Copy)]
//! struct Int(i32);
//!
//! struct Uint(u8);
//!
//! impl SaturatingElement<Uint> for Int {
//!     fn as_element(self) -> Uint {
//!         Uint(self.0.max(0).min(255) as u8)
//!     }
//! }
//!
//! impl SaturatingCast for Int {}
//!
//! assert_eq!(u8::MIN, Int(i32::MIN).saturating_cast::<Uint>().0);
//! assert_eq!(u8::MAX, Int(512).saturating_cast::<Uint>().0);
#![forbid(
    absolute_paths_not_starting_with_crate,
    missing_docs,
    non_ascii_idents,
    noop_method_call,
    unsafe_code,
    unused_results
)]
#![cfg_attr(not(test), no_std)]

mod saturate;
pub use saturate::{SaturatingCast, SaturatingElement};
