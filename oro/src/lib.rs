//! # Oro Module High Level API and Build Tooling
//!
//! # Usage
//! Declare the `oro` crate as a dependency in your `Cargo.toml` file.
//! You must also specify that you want Cargo to use a `build.rs` script.
//!
//! ```toml
//! [package]
//! build = "build.rs"
//!
//! [dependencies]
//! oro = { version = "<version>", features = ["runtime"] }
//!
//! [build-dependencies]
//! oro = { version = "<version>", features = ["build"] }
//! ```
//!
//! The `oro` crate *must* be `use`d by the module, even if no APIs are
//! called, in order to properly link the panic handler, etc.
//!
//! ```no_run
//! #[allow(unused_imports)]
//! use oro;
//! ```
//!
//! Finally, in your `build.rs`, call the `build` function to configure the linker to
//! generate a valid Oro module that can be loaded by the Oro kernel.
//!
//! ```no_run
//! fn main() {
//! 	::oro::build();
//! }
//! ```
#![cfg_attr(not(feature = "build"), no_std)]

#[cfg(feature = "build")]
mod build;
#[cfg(feature = "build")]
pub use build::*;

#[cfg(not(feature = "build"))]
mod runtime;
#[cfg(not(feature = "build"))]
pub use runtime::*;
