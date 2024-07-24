//! Common proc macros used by the `oro-common` crate.
//!
//! > **NOTE:** Do NOT use this crate directly. It is only intended
//! > to be used by the `oro-common` crate; anything meant to be used
//! > by other crates will be re-exported by `oro-common`.
#![deny(missing_docs, clippy::missing_docs_in_private_items)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]
// TODO(qix-): Remove this when <https://github.com/rust-lang/rust-clippy/issues/12425> is fixed
#![allow(clippy::tabs_in_doc_comments)]
#![feature(let_chains, proc_macro_span)]

use proc_macro2::{Span, TokenTree};
use quote::quote;
use syn::{punctuated::Punctuated, Error, Ident, Lit, Meta, Token};

/// Derive macro for the `EnumIterator` trait.
///
/// This macro generates an implementation of the `EnumIterator` trait
/// which allows you to iterate over all unit variants of an enum via the
/// `iter_all()` method.
///
/// All variants in the enum MUST have no fields ("unit variants").
///
/// # Example
///
/// ```rust
/// use oro_common::proc::EnumIterator;
///
/// #[derive(EnumIterator, Debug)]
/// enum MyEnum {
/// 	Variant1,
/// 	Variant2,
/// 	Variant3,
/// }
///
/// for variant in MyEnum::iter_all() {
/// 	println!("{:?}", variant);
/// }
/// ```
#[proc_macro_derive(EnumIterator)]
pub fn derive_enum_iterator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let ast = syn::parse_macro_input!(input as syn::DeriveInput);

	let name = &ast.ident;

	let syn::Data::Enum(data) = &ast.data else {
		return syn::Error::new_spanned(ast, "#[derive(EnumIterator)] is only valid for `enum`s")
			.to_compile_error()
			.into();
	};

	let mut variant_names = Vec::new();

	for variant in &data.variants {
		if !variant.fields.is_empty() {
			return syn::Error::new_spanned(
				variant,
				"#[derive(EnumIterator)] only works with enums with unit variants.",
			)
			.to_compile_error()
			.into();
		}

		variant_names.push(&variant.ident);
	}

	let expanded = quote! {
		const _: () = {
			struct EnumIteratorImpl {
				index: usize,
			}

			#[automatically_derived]
			impl ::oro_common::proc::EnumIterator for #name {
				fn iter_all() -> impl Iterator<Item = Self> + Sized + 'static {
					EnumIteratorImpl { index: 0 }
				}
			}

			#[automatically_derived]
			impl ::core::iter::Iterator for EnumIteratorImpl {
				type Item = #name;

				fn next(&mut self) -> Option<Self::Item> {
					const VARIANTS: &[#name] = &[#(#name::#variant_names),*];
					if self.index < VARIANTS.len() {
						let variant = VARIANTS[self.index];
						self.index += 1;
						Some(variant)
					} else {
						None
					}
				}
			}
		};
	};

	expanded.into()
}

/// Proc macro that provides pasting tokens together into identifiers.
///
/// Usage:
/// ```rust
/// paste! {
///    // These both generate a function named `foobar`
///    // (whitespace is ignored).
///    fn foo%%bar() {}
///    fn foo %% bar() {}
/// }
/// ```
///
/// All tokens are concatenated together into a single identifier.
/// Concatenated tokens MUST be identifiers.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
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

/// Derive macro that allows unit enums with designators to be safely
/// converted to/from a `u64`.
#[proc_macro_derive(AsU64)]
pub fn enum_as_u64(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let ast = syn::parse_macro_input!(input as syn::DeriveInput);

	let name = &ast.ident;

	let syn::Data::Enum(data) = &ast.data else {
		return syn::Error::new_spanned(ast, "#[derive(AsU64)] is only valid for `enum`s")
			.to_compile_error()
			.into();
	};

	// make sure it's a repr(u64) enum
	if !ast.attrs.iter().any(|attr| {
		let Meta::List(ref list) = attr.meta else {
			return false;
		};

		let mut tokens = list.tokens.clone().into_iter();
		let count = tokens.clone().count();
		let first = tokens.next();

		list.path.is_ident("repr")
			&& count == 1
			&& matches!(first, Some(TokenTree::Ident(first)) if first == "u64")
	}) {
		return syn::Error::new_spanned(
			ast,
			"#[derive(AsU64)] only works with enums that are repr(u64).",
		)
		.to_compile_error()
		.into();
	}

	let mut variant_matches = Vec::new();

	for variant in &data.variants {
		if !variant.fields.is_empty() {
			return syn::Error::new_spanned(
				variant,
				"#[derive(AsU64)] only works with enums with unit variants.",
			)
			.to_compile_error()
			.into();
		}

		let Some((_, discrim)) = &variant.discriminant.as_ref() else {
			return syn::Error::new_spanned(
				variant,
				"#[derive(AsU64)] only works with enums with explicit discriminants.",
			)
			.to_compile_error()
			.into();
		};
		let variant_ident = &variant.ident;

		variant_matches.push(quote! {
			#discrim => #name::#variant_ident,
		});
	}

	let expanded = quote! {
		#[automatically_derived]
		impl From<#name> for u64 {
			fn from(val: #name) -> u64 {
				val as u64
			}
		}

		#[automatically_derived]
		impl From<u64> for #name {
			fn from(val: u64) -> #name {
				match val {
					#(#variant_matches)*
					unknown => panic!("invalid value: {unknown:b}"),
				}
			}
		}
	};

	expanded.into()
}

/// Derive macro that allows unit enums with designators to be safely
/// converted to/from a `u32`.
#[proc_macro_derive(AsU32)]
pub fn enum_as_u32(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let ast = syn::parse_macro_input!(input as syn::DeriveInput);

	let name = &ast.ident;

	let syn::Data::Enum(data) = &ast.data else {
		return syn::Error::new_spanned(ast, "#[derive(AsU32)] is only valid for `enum`s")
			.to_compile_error()
			.into();
	};

	// make sure it's a repr(u32) enum
	if !ast.attrs.iter().any(|attr| {
		let Meta::List(ref list) = attr.meta else {
			return false;
		};

		let mut tokens = list.tokens.clone().into_iter();
		let count = tokens.clone().count();
		let first = tokens.next();

		list.path.is_ident("repr")
			&& count == 1
			&& matches!(first, Some(TokenTree::Ident(first)) if first == "u32")
	}) {
		return syn::Error::new_spanned(
			ast,
			"#[derive(AsU32)] only works with enums that are repr(u32).",
		)
		.to_compile_error()
		.into();
	}

	let mut variant_matches = Vec::new();

	for variant in &data.variants {
		if !variant.fields.is_empty() {
			return syn::Error::new_spanned(
				variant,
				"#[derive(AsU32)] only works with enums with unit variants.",
			)
			.to_compile_error()
			.into();
		}

		let Some((_, discrim)) = &variant.discriminant.as_ref() else {
			return syn::Error::new_spanned(
				variant,
				"#[derive(AsU32)] only works with enums with explicit discriminants.",
			)
			.to_compile_error()
			.into();
		};
		let variant_ident = &variant.ident;

		variant_matches.push(quote! {
			#discrim => #name::#variant_ident,
		});
	}

	let expanded = quote! {
		#[automatically_derived]
		impl From<#name> for u32 {
			fn from(val: #name) -> u32 {
				val as u32
			}
		}

		#[automatically_derived]
		impl From<u32> for #name {
			fn from(val: u32) -> #name {
				match val {
					#(#variant_matches)*
					unknown => panic!("invalid value: {unknown:b}"),
				}
			}
		}
	};

	expanded.into()
}

/// Loads a python script from a file and embeds it into the binary
/// as an inline GDB autoload script.
#[proc_macro]
pub fn gdb_autoload_inline(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let Some(tok) = input.clone().into_iter().next() else {
		return Error::new(Span::call_site(), "expected a string literal")
			.to_compile_error()
			.into();
	};
	let span = tok.span();
	let byte_span = span.byte_range();
	let source_path = span.source_file().path();

	let path_lit = syn::parse_macro_input!(input as syn::LitStr);

	let rel_path = path_lit.value();
	let rel_path = std::path::Path::new(&rel_path);

	// Resolve the path relative to the script that includes it
	let Some(source_dir) = source_path.parent() else {
		return Error::new(Span::call_site(), "failed to resolve source file directory")
			.to_compile_error()
			.into();
	};

	let manifest_path = source_dir.join(rel_path);

	let mut script = match std::fs::read(manifest_path) {
		Ok(b) => b,
		Err(e) => {
			return Error::new(
				Span::call_site(),
				format!("failed to load script: {e}: {rel_path:?}"),
			)
			.to_compile_error()
			.into();
		}
	};

	// Tell GDB this is an inline script
	script.splice(0..0, "gdb.inlined-script\n".bytes());

	// Prefix the `0x04` byte to indicate it's a literal python script
	script.insert(0, 0x04);

	// Postfix a null byte to indicate the end of the script
	script.push(0x00);

	// Create a byte array literal from the script
	let mut script_lit = Punctuated::<Lit, Token![,]>::new();
	let script_len = script.len();
	for b in script {
		script_lit.push(Lit::Int(syn::LitInt::new(
			&b.to_string(),
			Span::call_site(),
		)));
	}

	let script_ident = Ident::new(
		&format!("_SCRIPT_{}_{}", byte_span.start, byte_span.end),
		Span::call_site(),
	);

	let expanded = quote! {
			#[used]
			#[link_section = ".debug_gdb_scripts"]
			static #script_ident: [u8; #script_len] = {
					// This is unused, but creates a build-time dependency
					// on the file.
					include_bytes!(#path_lit);
					[ #script_lit ]
			};
	};

	expanded.into()
}
