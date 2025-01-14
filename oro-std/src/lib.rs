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
//! ## OS-Specific `std::os::oro` Module
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
#![no_std]

#[expect(unused_imports)]
use ::oro;

pub mod os;
pub mod prelude;
