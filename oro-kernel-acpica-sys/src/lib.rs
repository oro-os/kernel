//! Low level, Oro-specific Rust bindings to the ACPICA library.
//!
//! > **NOTE:** This crate does not export any C functionality;
//! > it is purely for data structures and constants.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]
#![expect(
	non_upper_case_globals,
	non_camel_case_types,
	non_snake_case,
	unsafe_op_in_unsafe_fn,
	rustdoc::bare_urls,
	clippy::missing_safety_doc
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
include!(concat!(env!("OUT_DIR"), "/tablegen_macro.rs"));
