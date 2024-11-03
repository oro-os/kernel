//! Common memory types and functions for the Oro kernel.
#![cfg_attr(not(test), no_std)]
#![expect(internal_features)]
#![feature(core_intrinsics, never_type)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

#[cfg(all(not(feature = "std-alloc"), not(test)))]
extern crate alloc as _;

pub mod mapper;
pub mod pfa;
pub mod phys;
pub mod translate;

#[cfg(all(not(feature = "std-alloc"), not(test)))]
pub mod alloc;
