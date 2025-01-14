//! # Oro Interim `std` Library
//!
//! > **NOTE:** This crate is intended to be used as a temporary stand-in for the Rust standard library
//! > when targeting the Oro kernel. It is not intended to be a full replacement for the Rust standard
//! > library, and is definitely missing a _lot_ of functionality.
//! >
//! > Any functionality that *is* implemented should match the behavior of the Rust standard library
//! > exactly. Any deviations or _additional_ functionality not present in mainline `std`
//! > should be considered a bug and reported.
//!
//! ## Usage
//! Declare the `std` crate as a dependency in your `Cargo.toml` file, mapping it to `oro-std`:
//!
//! ```toml
//! [dependencies]
//! std = { git = "https://github.com/oro-os/kernel.git", package = "oro-std" }
//! ```
//!
//! ### OS-Specific `std::os::oro` Module
//! By default, the `std::os::oro` module is **not** enabled. To enable it, you must include the
//! `oro` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! std = { git = "https://github.com/oro-os/kernel.git", package = "oro-std", features = ["oro"] }
//! ```
//!
//! Note that `std::os::oro` is just a re-export of the `oro` crate. If you wish to write
//! less-fragile code for the future, you may choose to depend on `oro` directly.
//!
//! ### Nightly Features
//! To enable certain nightly-only features that also exist in `std`, you can enable the `nightly`
//! feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! std = { git = "https://github.com/oro-os/kernel.git", package = "oro-std", features = ["nightly"] }
//! ```
//!
//! Further, any feature typically enabled with `#![feature(...)]` in nightly Rust can be enabled
//! in `oro-std` by adding the feature to the `Cargo.toml` file. **Any `core` features must be
//! included using the normal `#![feature(...)]` syntax in your code.** This may change in the
//! future.
//!
//! > **NOTE:** Using the `oro-std` crates **always requires nightly** Rust, as the `oro` crate uses
//! > unstable features. However, this doesn't intrinsically mean application code also wants to
//! > use nightly features. Therefore, the `nightly` feature is **not** automatically enabled simply
//! > by building using a nightly toolchain.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
#![feature(cfg_target_thread_local)]

#[expect(unused_imports)]
use ::oro;

pub mod os;
pub mod prelude;
pub mod thread;

pub use core::{
	any, arch, array, ascii, borrow, cell, char, clone, cmp, convert, default, f32, f64, ffi, fmt,
	hash, hint, iter, marker, num, ops, option, pin, primitive, ptr, result, slice, str,
};
