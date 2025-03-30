//! Macros (including proc macros) and supporting types for the Oro kernel.
//!
//! This crate also re-exports and provides the supplementary types
//! for the [`oro_macro_proc`] crate.
//!
//! It also houses the tests for the procedural macros.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

#[cfg(test)]
mod tests;

pub mod asm_buffer;
pub mod assert;
pub mod unsafe_macros;

pub use oro_macro_proc::*;

// We re-export this at the top level so that both
// the derive macro and trait get imported at once.
mod enum_iterator;
pub use enum_iterator::EnumIterator;
