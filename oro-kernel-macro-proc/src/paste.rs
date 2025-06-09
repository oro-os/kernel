//! Core functionality for the `paste!()` macro.
#![expect(clippy::missing_docs_in_private_items)]

use std::collections::VecDeque;

use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenTree};
use syn::{Error, Ident};

#[expect(clippy::missing_docs_in_private_items, clippy::too_many_lines)]
pub fn paste(input: proc_macro::TokenStream) -> syn::Result<proc_macro::TokenStream> {
	let input = proc_macro2::TokenStream::from(input);
	let mut iter = CaseTransformationIterator::new(input.into_iter()).peekable();
	let mut output = proc_macro2::TokenStream::new();

	while let Some(token) = iter.next().transpose()? {
		// Now handle the token itself
		match token {
			TokenTree::Group(group) => {
				let new_group = TokenTree::Group(proc_macro2::Group::new(
					group.delimiter(),
					paste(group.stream().into())?.into(),
				));
				output.extend([new_group]);
			}
			TokenTree::Ident(ident) => {
				if let Some(TokenTree::Punct(punct_peek)) = iter.peek().cloned().transpose()?
					&& punct_peek.as_char() == '%'
				{
					let mut iter_branch = iter.clone();

					let mut idents = vec![(ident.to_string(), ident.span())];
					let mut additional = None;

					while let Some(TokenTree::Punct(punct_first)) =
						iter_branch.peek().cloned().transpose()?
						&& punct_first.as_char() == '%'
					{
						let punct_first = iter_branch
							.next()
							.transpose()?
							.expect("peek gave a value for punct_first but next() returned None");

						if let Some(TokenTree::Punct(punct_second)) =
							iter_branch.peek().cloned().transpose()?
							&& punct_second.as_char() == '%'
							&& punct_first
								.span()
								.join(punct_second.span())
								.and_then(|s| s.source_text())
								.is_some_and(|s| s == "%%")
						{
							iter_branch.next().transpose()?.expect(
								"peek gave a value for punct_second but next() returned None",
							);

							let maybe_token = iter_branch.next().transpose()?;

							// Unwrap groups if they have a single token.
							let maybe_token = match maybe_token {
								Some(TokenTree::Group(group)) => {
									let mut iter_group = group.stream().into_iter();
									if let Some(token) = iter_group.next() {
										if iter_group.next().is_none() {
											Some(token)
										} else {
											Some(TokenTree::Group(group))
										}
									} else {
										Some(TokenTree::Group(group))
									}
								}
								other => other,
							};

							match maybe_token {
								Some(TokenTree::Ident(ident)) => {
									idents.push((ident.to_string(), ident.span()));
								}
								Some(TokenTree::Literal(lit)) => {
									idents.push((lit.to_string(), lit.span()));
								}
								unexpected => {
									return Err(Error::new(
										unexpected
											.as_ref()
											.map_or_else(Span::call_site, TokenTree::span),
										format!(
											"expected identifier after `%%`; found: {unexpected:?}"
										),
									));
								}
							}
						} else {
							// This is something else, so we just output the `#` and continue
							additional = Some(punct_first);
							break;
						}
					}

					iter = iter_branch;

					output.extend([TokenTree::Ident(Ident::new(
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

	Ok(output.into())
}

/// Transforms a token stream's identifiers to snake_case.
#[derive(Clone)]
struct CaseTransformationIterator<I>
where
	I: Iterator<Item = TokenTree>,
{
	inner: I,
	cache: VecDeque<TokenTree>,
}

impl<I> CaseTransformationIterator<I>
where
	I: Iterator<Item = TokenTree>,
{
	fn new(inner: I) -> Self {
		Self {
			inner,
			cache: VecDeque::default(),
		}
	}

	fn next_cached(&mut self) -> Option<TokenTree> {
		let next = self.inner.next()?;
		self.cache.push_back(next.clone());
		Some(next)
	}
}

impl<I> Iterator for CaseTransformationIterator<I>
where
	I: Iterator<Item = TokenTree>,
{
	type Item = syn::Result<TokenTree>;

	fn next(&mut self) -> Option<Self::Item> {
		// flush whatever is in the cache
		if let Some(token) = self.cache.pop_front() {
			return Some(Ok(token));
		}

		macro_rules! parse_ahead {
			(($ident:tt, $span:tt) => String) => {
				let Some(v) = self.next_cached() else {
					return Some(Ok(self.cache.pop_front()?));
				};

				let ($ident, $span) = if let TokenTree::Ident(v) = v {
					(v.to_string(), v.span())
				} else {
					return Some(Ok(self.cache.pop_front()?));
				};
			};

			(($ident:tt, $span:tt) => char $c:literal) => {
				let Some(v) = self.next_cached() else {
					return Some(Ok(self.cache.pop_front()?));
				};

				let ($ident, $span) = if let TokenTree::Punct(v) = v {
					if v.as_char() == $c {
						(v.as_char(), v.span())
					} else {
						return Some(Ok(self.cache.pop_front()?));
					}
				} else {
					return Some(Ok(self.cache.pop_front()?));
				};
			};
		}

		parse_ahead! { (_, _) => char '%' };
		parse_ahead! { (_, _) => char '<' };
		parse_ahead! { (op, op_span) => String };
		parse_ahead! { (_, _) => char ':' };
		parse_ahead! { (ident, ident_span) => String };
		parse_ahead! { (_, _) => char '>' };
		parse_ahead! { (_, _) => char '%' };

		self.cache.clear();

		match op.as_str() {
			"snake_case" => {
				let ident = ident.to_string();
				let snake_case = ident.to_case(Case::Snake);
				let ident = Ident::new(&snake_case, ident_span);
				Some(Ok(TokenTree::Ident(ident)))
			}
			"title_case" => {
				let ident = ident.to_string();
				let title_case = ident.to_case(Case::Pascal);
				let ident = Ident::new(&title_case, ident_span);
				Some(Ok(TokenTree::Ident(ident)))
			}
			"camel_case" => {
				let ident = ident.to_string();
				let camel_case = ident.to_case(Case::Camel);
				let ident = Ident::new(&camel_case, ident_span);
				Some(Ok(TokenTree::Ident(ident)))
			}
			"const_case" => {
				let ident = ident.to_string();
				let const_case = ident.to_case(Case::ScreamingSnake);
				let ident = Ident::new(&const_case, ident_span);
				Some(Ok(TokenTree::Ident(ident)))
			}
			unknown => {
				Some(Err(Error::new(
					op_span,
					format!("unknown `paste!` operation: {unknown}"),
				)))
			}
		}
	}
}
