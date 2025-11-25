//! Core functionality for the `#[derive(EnumAs_)]` proc macros.

use proc_macro2::TokenTree;
use quote::quote;
use syn::Meta;

#[expect(clippy::missing_docs_in_private_items)]
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
			k if k == #discrim => Ok(#name::#variant_ident),
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
		impl TryFrom<u64> for #name {
			type Error = u64;

			fn try_from(val: u64) -> ::core::result::Result<Self, Self::Error> {
				match val {
					#(#variant_matches)*
					unknown => Err(unknown),
				}
			}
		}
	};

	expanded.into()
}

#[expect(clippy::missing_docs_in_private_items)]
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
			#discrim => Ok(#name::#variant_ident),
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
		impl TryFrom<u32> for #name {
			type Error = u32;

			fn try_from(val: u32) -> ::core::result::Result<Self, Self::Error> {
				match val {
					#(#variant_matches)*
					unknown => Err(unknown),
				}
			}
		}
	};

	expanded.into()
}
