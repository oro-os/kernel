#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

mod address_scheme;
mod page_size;

pub use self::{
	address_scheme::{ArchAddressScheme, CheckUnsafePhys, CheckUnsafeVirt, UnsafePhys, UnsafeVirt},
	page_size::{ArchPageSize, PageSize},
};
