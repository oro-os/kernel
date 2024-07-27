//! Core functionality for the `paste!()` macro.

use proc_macro2::{Span, TokenTree};
use syn::{Error, Ident};

#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_docs_in_private_items)]
pub fn paste(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = proc_macro2::TokenStream::from(input);
	let mut iter = input.into_iter().peekable();
	let mut output = proc_macro2::TokenStream::new();

	while let Some(token) = iter.next() {
		match token {
			TokenTree::Group(group) => {
				let new_group = TokenTree::Group(proc_macro2::Group::new(
					group.delimiter(),
					paste(group.stream().into()).into(),
				));
				output.extend([new_group]);
			}
			TokenTree::Ident(ident) => {
				if let Some(TokenTree::Punct(punct_peek)) = iter.peek()
					&& punct_peek.as_char() == '%'
				{
					let mut iter_branch = iter.clone();

					let mut idents = vec![(ident.to_string(), ident.span())];
					let mut additional = None;

					while let Some(TokenTree::Punct(punct_first)) = iter_branch.peek()
						&& punct_first.as_char() == '%'
					{
						let punct_first = iter_branch
							.next()
							.expect("peek gave a value for punct_first but next() returned None");

						if let Some(TokenTree::Punct(punct_second)) = iter_branch.peek()
							&& punct_second.as_char() == '%'
							&& punct_first
								.span()
								.join(punct_second.span())
								.and_then(|s| s.source_text())
								.is_some_and(|s| s == "%%")
						{
							iter_branch.next().expect(
								"peek gave a value for punct_second but next() returned None",
							);

							match iter_branch.next() {
								Some(TokenTree::Ident(ident)) => {
									idents.push((ident.to_string(), ident.span()));
								}
								Some(TokenTree::Literal(lit)) => {
									idents.push((lit.to_string(), lit.span()));
								}
								unexpected => {
									return Error::new(
										unexpected
											.as_ref()
											.map_or_else(Span::call_site, TokenTree::span),
										format!(
											"expected identifier after `%%`; found: {unexpected:?}"
										),
									)
									.to_compile_error()
									.into();
								}
							}
						} else {
							// This is something else, so we just output the `#` and continue
							additional = Some(punct_first);
							break;
						}
					}

					iter = iter_branch;

					output.extend([TokenTree::Ident(Ident::new_raw(
						&idents
							.iter()
							.map(|(ident, _)| ident.clone())
							.collect::<String>(),
						idents
							.iter()
							.map(|(_, span)| *span)
							.reduce(|l, r| l.join(r).unwrap_or(l))
							.expect("no spans to join"),
					))]);

					if let Some(additional) = additional {
						output.extend([additional]);
					}
				} else {
					output.extend([TokenTree::Ident(ident)]);
				}
			}
			other => {
				output.extend([other]);
			}
		}
	}

	output.into()
}
