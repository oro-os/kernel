//! Procedural macros and supporting types for the Oro kernel.
//!
//! This crate re-exports and provides the supplementary types
//! for the [`oro_common_proc_macros`] crate.
//!
//! It also houses the tests for the procedural macros.
#![cfg_attr(not(test), no_std)]

#[cfg(test)]
mod tests;

pub use oro_common_proc_macros::*;
