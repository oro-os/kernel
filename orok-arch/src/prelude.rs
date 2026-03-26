//! Prelude module for use by consumers of `orok-arch`.
#![expect(
	clippy::unsafe_removed_from_name,
	reason = "False positive; https://github.com/rust-lang/rust-clippy/issues/16768"
)]

pub use orok_arch_base::{
	CheckUnsafePhys as _, CheckUnsafeVirt as _, PageSize as _, UnsafePhys as _, UnsafeVirt as _,
};
