//! Simple primitive types and associated traits.
//!
//! This crate consists more or less of primitive type
//! wrappers and associated traits for them (e.g.
//! forced endianness types).
#![cfg_attr(not(test), no_std)]
#![expect(clippy::inline_always, clippy::wrong_self_convention)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

mod endian;
mod volatile;

pub use self::{endian::*, volatile::*};
