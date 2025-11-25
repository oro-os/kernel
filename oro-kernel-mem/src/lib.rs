//! Common memory types and functions for the Oro kernel.
#![cfg_attr(not(test), no_std)]
#![expect(internal_features)]
#![feature(core_intrinsics, never_type)]
#![cfg_attr(doc, feature(doc_cfg))]

pub extern crate alloc;

pub mod mapper;
pub mod pfa;
pub mod phys;
pub mod translate;

pub mod global_alloc;
