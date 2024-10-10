//! Common memory types and functions for the Oro kernel.
#![cfg_attr(not(test), no_std)]
#![expect(internal_features)]
#![feature(core_intrinsics, never_type)]
#![cfg_attr(debug_assertions, feature(naked_functions))]

pub mod mapper;
pub mod pfa;
pub mod phys;
pub mod region;
pub mod translate;

oro_macro::oro_global_getter! {
	pub(crate) pfa -> crate::pfa::alloc::GlobalPfa,
	pub(crate) translator -> crate::translate::Translate,
}
