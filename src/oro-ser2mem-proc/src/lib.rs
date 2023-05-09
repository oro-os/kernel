//! See the crate comment in [oro-ser2mem] for more information
//! about this crate, as this is purely a formality crate due
//! due Rust's requirements about separating proc-macro crates.

extern crate proc_macro;

use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, TokenStreamExt};
use syn::{parse_macro_input, spanned::Spanned, Error, Ident, ItemStruct, Meta, Visibility};

#[proc_macro_derive(Ser2Mem)]
pub fn derive_ser2mem_structure(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut structure = parse_macro_input!(item as ItemStruct);

	'find_repr: {
		for attr in &structure.attrs {
			if let Meta::List(ref maybe_repr) = &attr.meta {
				if maybe_repr.path.leading_colon.is_none()
					&& maybe_repr.path.segments.len() == 1
					&& maybe_repr.path.segments.first().unwrap().ident == "repr"
					&& !maybe_repr.tokens.is_empty()
					&& {
						let first_token = maybe_repr.tokens.clone().into_iter().next().unwrap();
						match first_token {
							TokenTree::Ident(i) => i == "C",
							_ => false,
						}
					} {
					break 'find_repr;
				}
			}
		}

		return Error::new(
			structure.span(),
			"ser2mem structures must be annotated as #[repr(C)]",
		)
		.into_compile_error()
		.into();
	}

	let mut field_writes: Vec<TokenStream> = Vec::new();
	for ref field in &mut structure.fields {
		if !matches!(field.vis, Visibility::Public(_)) {
			return Error::new(field.span(), "all ser2mem fields must be `pub`")
				.into_compile_error()
				.into();
		}

		let field_ident = &field.ident;

		field_writes.push(quote! {
			self.#field_ident.serialize_to(&mut target.#field_ident as *mut _, alloc);
		});
	}

	let field_writes = field_writes.into_iter().reduce(|mut acc, e| {
		acc.append_all(e.into_iter());
		acc
	});

	let orig_ident = structure.ident;
	let proxy_ident = Ident::new(&format!("{}Proxy", orig_ident), orig_ident.span());
	structure.ident = proxy_ident.clone();

	quote! {
		const _: () = {
			#structure

			#[automatically_derived]
			unsafe impl ::oro_ser2mem::_detail::Proxied for #orig_ident {
				type Proxy = #proxy_ident;
			}

			#[automatically_derived]
			unsafe impl ::oro_ser2mem::_detail::Proxy for #proxy_ident {
				unsafe fn serialize<A>(self, alloc: &mut A) where A: ::oro_ser2mem::Allocator {
					const layout: ::core::alloc::Layout = ::core::alloc::Layout::new::<#orig_ident>();
					debug_assert_eq!(alloc.position() % layout.align() as u64, 0);
					let target = &mut *(alloc.position() as *mut Self);
					alloc.allocate(layout.size() as u64);
					use ::oro_ser2mem::_detail::Serializable;
					#field_writes
				}
			}
		};
	}
	.into()
}
