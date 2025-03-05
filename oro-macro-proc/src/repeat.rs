#![expect(clippy::missing_docs_in_private_items)]

use proc_macro2::TokenStream;
use syn::{
	LitInt, Result, Token, braced,
	parse::{Parse, ParseStream},
	token::Brace,
};

struct Repeat {
	count:   LitInt,
	_arrow:  Token![=>],
	_braces: Brace,
	block:   TokenStream,
}

impl Parse for Repeat {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		let content;
		Ok(Self {
			count:   input.parse()?,
			_arrow:  input.parse()?,
			_braces: braced!(content in input),
			block:   content.parse()?,
		})
	}
}

pub fn repeat(input: proc_macro::TokenStream) -> Result<TokenStream> {
	let repdef: Repeat = syn::parse(input)?;

	let iter = repdef.block.into_iter();
	let count = iter.clone().count();

	Ok(iter
		.cycle()
		.take(count * repdef.count.base10_parse::<usize>()?)
		.collect::<TokenStream>())
}
