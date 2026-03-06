#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]
#![feature(type_alias_impl_trait)]

mod address;
mod arch_types;
pub mod prelude;

pub use orok_arch_base::{CheckUnsafePhys, CheckUnsafeVirt};

pub use self::{
	address::{Phys, Virt},
	arch_types::*,
};
