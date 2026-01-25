#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(doc, feature(doc_cfg))]

extern crate proc_macro;

mod effect;

use proc_macro::TokenStream;

/// Declares an effect on a function.
///
/// Effects are used for debugging in order to track side effects,
/// pre- and post-conditions, and other runtime behaviors via runtime
/// analysis implementations (namely during testing or local development).
///
/// They have no effect on release builds unless explicitly enabled.
#[proc_macro_attribute]
pub fn effect(attr: TokenStream, input: TokenStream) -> TokenStream {
	match effect::effect(attr.into(), input.into()) {
		Ok(ts) => ts.into(),
		Err(err) => err.to_compile_error().into(),
	}
}
