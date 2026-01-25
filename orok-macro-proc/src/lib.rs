#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(doc, feature(doc_cfg))]
#![feature(proc_macro_diagnostic, proc_macro_span)]
#![expect(
	clippy::single_call_fn,
	reason = "most macro entry points are single-call for code organization"
)]
#![allow(
	clippy::arithmetic_side_effects,
	clippy::unwrap_used,
	clippy::unwrap_in_result,
	clippy::indexing_slicing,
	clippy::unreachable,
	reason = "panics during proc macro expansion are acceptable"
)]
#![allow(
	clippy::missing_docs_in_private_items,
	reason = "macros are typically self-documenting through public interface"
)]
#![allow(
	clippy::needless_pass_by_value,
	reason = "consistent ownership semantics for proc macro inputs"
)]
#![allow(
	clippy::mixed_read_write_in_expression,
	reason = "common pattern when `syn`-parsing braces or other containers"
)]

extern crate proc_macro;

mod bitstruct;
mod blanket_trait;

use proc_macro::TokenStream;

/// Implements a blanket trait for all types `T` that
/// satisfy the trait bounds specified in the blanket trait definition.
///
/// ```ignore
/// use orok_macro::BlanketTrait;
///
/// #[blanket_trait]
/// pub trait MyBlanketTrait : SomeBound + AnotherBound {}
/// ```
///
/// This generates the following:
///
/// ```ignore
/// impl<T> MyBlanketTrait for T where T: SomeBound + AnotherBound {}
/// ```
#[proc_macro_attribute]
pub fn blanket_trait(attr: TokenStream, input: TokenStream) -> TokenStream {
	match blanket_trait::blanket_trait(attr.into(), input.into()) {
		Ok(ts) => ts.into(),
		Err(err) => err.to_compile_error().into(),
	}
}

/// Defines a bit structure wrapper type around a primitive integer type,
/// along with a set of field accessors, associated constants, and other utility functionality.
#[proc_macro]
pub fn bitstruct(input: TokenStream) -> TokenStream {
	match bitstruct::bitstruct(input) {
		Ok(ts) => ts,
		Err(err) => err.to_compile_error().into(),
	}
}

