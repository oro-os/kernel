//! Common memory types and functions for the Oro kernel.
#![cfg_attr(not(test), no_std)]
#![expect(internal_features)]
#![feature(core_intrinsics, never_type)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

// TODO(qix-): This crate was originally a kitchen sink
// TODO(qix-): "commons" crate for the entire kernel project.
// TODO(qix-): It's since been split up into multiple crates,
// TODO(qix-): leaving this one as the "memory" crate.
//
// TODO(qix-): There may still be some documentation and comments
// TODO(qix-): that refer to the old "commons" crate, but they
// TODO(qix-): should be updated as time goes on.

pub mod mapper;
pub mod pfa;
pub mod region;
pub mod translate;
