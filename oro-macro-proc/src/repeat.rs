#![expect(clippy::missing_docs_in_private_items)]

use std::iter::Peekable;

use proc_macro2::{Group, Literal, Spacing, TokenStream, TokenTree};
use syn::{
	Ident, LitInt, RangeLimits, Result, Token, braced,
	parse::{Parse, ParseStream},
	token::Brace,
};

struct AsSpec {
	_dollar: Token![$],
	ident:   Ident,
}

impl Parse for AsSpec {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		Ok(Self {
			_dollar: input.parse()?,
			ident:   input.parse()?,
		})
	}
}

enum DelimSpec {
	Underscore,
	AsSpec(AsSpec),
}

impl Parse for DelimSpec {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		if input.peek(Token![_]) {
			let _us: Token![_] = input.parse().unwrap();
			Ok(DelimSpec::Underscore)
		} else {
			Ok(DelimSpec::AsSpec(input.parse()?))
		}
	}
}

struct RangeSpec {
	from:   Option<LitInt>,
	limits: RangeLimits,
	to:     LitInt,
}

impl Parse for RangeSpec {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		Ok(Self {
			from:   if input.peek(Token![..]) || input.peek(Token![..=]) {
				None
			} else {
				Some(input.parse()?)
			},
			limits: input.parse()?,
			to:     input.parse()?,
		})
	}
}

impl RangeSpec {
	fn range(&self) -> Result<(usize, usize)> {
		let start = self.from.as_ref().map_or(Ok(0), |t| t.base10_parse())?;

		let mut end = self.to.base10_parse()?;

		if matches!(self.limits, RangeLimits::Closed(_)) {
			end += 1;
		}

		Ok((start, end))
	}
}

struct Repeat {
	delim_spec: DelimSpec,
	_in:        Token![in],
	range:      RangeSpec,
	_braces:    Brace,
	block:      TokenStream,
}

impl Repeat {
	fn repeat_token(&self) -> Option<Ident> {
		if let DelimSpec::AsSpec(aspec) = &self.delim_spec {
			Some(aspec.ident.clone())
		} else {
			None
		}
	}
}

impl Parse for Repeat {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let content;
		Ok(Self {
			delim_spec: input.parse()?,
			_in:        input.parse()?,
			range:      input.parse()?,
			_braces:    braced!(content in input),
			block:      content.parse()?,
		})
	}
}

pub fn repeat(input: proc_macro::TokenStream) -> Result<TokenStream> {
	let repdef: Repeat = syn::parse(input)?;
	let (start, total) = repdef.range.range()?;
	let ident = repdef.repeat_token();
	Ok(repdef
		.block
		.into_iter()
		.repeat_count(start, total, ident)
		.collect())
}

trait IntoRepeatIterator: Iterator<Item = TokenTree> + Sized {
	fn repeat_count(self, start: usize, count: usize, ident: Option<Ident>)
	-> RepeatIterator<Self>;
	fn expand_count(self, count: usize, ident: Option<Ident>) -> ExpandIterator<Self>;
}

struct RepeatIterator<I: Iterator<Item = TokenTree>> {
	iter:          I,
	cur_iter:      ExpandIterator<I>,
	count:         usize,
	current_count: usize,
	ident:         Option<Ident>,
}

impl<I: Iterator<Item = TokenTree> + Clone> IntoRepeatIterator for I {
	fn repeat_count(
		self,
		start: usize,
		count: usize,
		ident: Option<Ident>,
	) -> RepeatIterator<Self> {
		RepeatIterator {
			iter: self.clone(),
			cur_iter: self.expand_count(start, ident.clone()),
			count,
			current_count: start,
			ident,
		}
	}

	fn expand_count(self, count: usize, ident: Option<Ident>) -> ExpandIterator<Self> {
		ExpandIterator {
			iter: self.peekable(),
			current_count: count,
			ident,
		}
	}
}

impl<I: Iterator<Item = TokenTree> + Clone> Iterator for RepeatIterator<I> {
	type Item = TokenTree;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(tt) = self.cur_iter.next() {
			return Some(tt);
		}

		self.current_count += 1;
		if self.current_count >= self.count {
			return None;
		}

		self.cur_iter = self
			.iter
			.clone()
			.expand_count(self.current_count, self.ident.clone());

		self.next()
	}
}

struct ExpandIterator<I: Iterator<Item = TokenTree>> {
	iter:          Peekable<I>,
	current_count: usize,
	ident:         Option<Ident>,
}

impl<I: Iterator<Item = TokenTree>> Iterator for ExpandIterator<I> {
	type Item = TokenTree;

	fn next(&mut self) -> Option<Self::Item> {
		let next = self.iter.next()?;

		if let Some(delim_ident) = &self.ident {
			if let TokenTree::Group(grp) = &next {
				return Some(TokenTree::Group(Group::new(
					grp.delimiter(),
					grp.stream()
						.into_iter()
						.expand_count(self.current_count, self.ident.clone())
						.collect(),
				)));
			}

			if let TokenTree::Punct(punct) = &next {
				if punct.as_char() == '$' && punct.spacing() == Spacing::Alone {
					if let Some(TokenTree::Ident(ident)) = self.iter.peek() {
						if ident == delim_ident {
							self.iter.next();
							return Some(TokenTree::Literal(Literal::usize_unsuffixed(
								self.current_count,
							)));
						}
					}
				}
			}
		}

		Some(next)
	}
}
