//! See the crate comment in [oro-ser2mem] for more information
//! about this crate, as this is purely a formality crate due
//! due Rust's requirements about separating proc-macro crates.

extern crate proc_macro;

use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, TokenStreamExt};
use std::collections::HashMap;
use syn::{
	parse::{discouraged::Speculative, Parse},
	parse_macro_input,
	spanned::Spanned,
	Error, GenericParam, Generics, Ident, ItemEnum, ItemStruct, Meta, TraitBoundModifier, Type,
	TypeParamBound, Visibility, WherePredicate,
};

enum StructOrEnum {
	Struct(ItemStruct),
	Enum(ItemEnum),
}

impl Parse for StructOrEnum {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let forked = input.fork();
		// The speculative parse here is indeed discouraged for very good reasons.
		// However, ser2mem is being used in such a controlled manner (friendly
		// reminder: SER2MEM IS NOT A GENERALIZED SERIALIZATION FRAMEWORK) that
		// we can accept this tradeoff of simpler code here vs. more advanced,
		// complex Rust parsing code.
		//
		// If you disagree, feel free to make a PR. However the maintenance cost
		// will be heavily debated.
		ItemStruct::parse(&forked).map_or_else(
			|_| ItemEnum::parse(input).map(StructOrEnum::Enum),
			|s| {
				input.advance_to(&forked);
				Ok(StructOrEnum::Struct(s))
			},
		)
	}
}

fn derive_struct(mut structure: ItemStruct) -> proc_macro::TokenStream {
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
			"ser2mem structs must be annotated as #[repr(C)]",
		)
		.into_compile_error()
		.into();
	}

	// Iterate over any generics for the struct, which should all be iterator
	// types. ser2mem structures do not support general-purpose generic types.
	let mut generic_map: HashMap<Ident, Option<TypeParamBound>> = HashMap::new();
	{
		let gen = &structure.generics;

		for param in gen.params.iter() {
			match param {
				GenericParam::Lifetime(lt) => {
					return Error::new(
						lt.span(),
						"lifetime parameters are not allowed in ser2mem generics",
					)
					.into_compile_error()
					.into()
				}
				GenericParam::Const(c) => {
					return Error::new(
						c.span(),
						"constant parameters are not allowed in ser2mem generics",
					)
					.into_compile_error()
					.into()
				}
				GenericParam::Type(gt) => {
					if !gt.attrs.is_empty() {
						return Error::new(
							gt.span(),
							"ser2mem generic types cannot have additional attributes",
						)
						.into_compile_error()
						.into();
					}

					if generic_map.contains_key(&gt.ident) {
						return Error::new(gt.ident.span(), "duplicate generic type param")
							.into_compile_error()
							.into();
					}

					match gt.bounds.len() {
						0 => generic_map.insert(gt.ident.clone(), None),
						1 => generic_map.insert(gt.ident.clone(), Some(gt.bounds[0].clone())),
						n => return Error::new(gt.bounds.span(), format!("ser2mem generic bounds can only contain a single bound (an `::oro_ser2mem::CloneIterator`); found {n}")).into_compile_error().into()
					};
				}
			}
		}

		// Attempt to enrich any generic types with the where clause.
		if let Some(wh) = &gen.where_clause {
			for predicate in wh.predicates.iter() {
				match predicate {
					WherePredicate::Lifetime(lt) => {
						return Error::new(
							lt.span(),
							"ser2mem generic `where` clauses cannot contain lifetime predicates",
						)
						.into_compile_error()
						.into()
					}
					WherePredicate::Type(gt) => {
						if gt.lifetimes.is_some() {
							return Error::new(gt.lifetimes.span(), "ser2mem generic `where` predicates cannot contain `for<'_>` lifetimes").into_compile_error().into();
						}

						let ident = match &gt.bounded_ty {
							Type::Path(pth) => {
								if let Some(qself) = &pth.qself {
									// XXX: https://github.com/dtolnay/syn/issues/1465
									return Error::new(qself.lt_token.span(), "ser2mem generic `where` predicates cannot use qualified type bounds").into_compile_error().into();
								}

								if pth.path.leading_colon.is_some() || pth.path.segments.len() > 1 {
									return Error::new(pth.path.span(), "ser2mem generic `where` predicate paths must only refer to generic parameters").into_compile_error().into();
								}

								// Should never be 0, and we already checked above that it's not >1
								debug_assert!(pth.path.segments.len() == 1);

								pth.path.segments[0].ident.clone()
							},
							other => return Error::new(other.span(), "ser2mem generic `where` predicates may only bound generic paramters to `::oro_ser2mem::CloneIterator`").into_compile_error().into()
						};

						if !generic_map.contains_key(&ident) {
							return Error::new(ident.span(), "unknown generic parameter")
								.into_compile_error()
								.into();
						}

						match gt.bounds.len() {
							0 => return Error::new(gt.bounds.span(), "ser2mem generic `where` predicates must have exactly 1 bound").into_compile_error().into(),
							1 => {generic_map.insert(ident.clone(), Some(gt.bounds[0].clone()))},
							n => return Error::new(gt.bounds.span(), format!("ser2mem generic bounds can only contain a single bound (an `::oro_ser2mem::CloneIterator`); found {n}")).into_compile_error().into()
						};
					}
					unknown => {
						return Error::new(
							unknown.span(),
							"unknown `where` predicate (and thus unsupported by ser2mem)",
						)
						.into_compile_error()
						.into()
					}
				}
			}
		}
	}

	let mut field_writes: Vec<TokenStream> = Vec::new();
	for ref mut field in &mut structure.fields {
		if !matches!(field.vis, Visibility::Public(_)) {
			return Error::new(field.span(), "all ser2mem fields must be `pub`")
				.into_compile_error()
				.into();
		}

		let field_ident = &field.ident;

		let new_type = if let Type::Path(pth) = &field.ty {
			if pth.path.leading_colon.is_none() && pth.path.segments.len() == 1 {
				let ident = &pth.path.segments[0].ident;
				match generic_map.get(ident) {
					None => None,
					Some(Some(TypeParamBound::Trait(pt))) => {
						match pt.modifier {
							TraitBoundModifier::None => {},
							_ => return Error::new(pt.modifier.span(), "ser2mem structure generic parameter bounds cannot be modified (e.g. with `?`)").into_compile_error().into()
						}

						if let Some(lts) = &pt.lifetimes {
							return Error::new(
								lts.span(),
								"ser2mem structure generic parameter bounds may not have lifetimes",
							)
							.into_compile_error()
							.into();
						}

						let tyb = &pt.path;

						Some(Type::Verbatim(quote! {
							&'static [<<(dyn #tyb) as Iterator>::Item as Serializable>::Target]
						}))
					}
					Some(Some(pt)) => {
						return Error::new(
							pt.span(),
							"ser2mem generic parameters must be bounded only to `::oro_ser2mem::CloneIterator` traits",
						)
						.into_compile_error()
						.into()
					}
					Some(None) => {
						return Error::new(
							ident.span(),
							"ser2mem generic type bounds must be trait bounds",
						)
						.into_compile_error()
						.into()
					}
				}
			} else {
				None
			}
		} else {
			None
		};

		if let Some(nt) = new_type {
			field.ty = nt;

			field_writes.push(quote! {
				target.#field_ident = ::oro_ser2mem::_detail::serialize_iterator_to_slice(
					self.#field_ident,
					alloc
				);
			});
		} else {
			field_writes.push(quote! {
				self.#field_ident.serialize_to(&mut target.#field_ident as *mut _, alloc);
			});
		}
	}

	let field_writes = field_writes.into_iter().reduce(|mut acc, e| {
		acc.append_all(e.into_iter());
		acc
	});

	let orig_ident = structure.ident;
	let proxy_ident = Ident::new(&format!("{}Proxy", orig_ident), orig_ident.span());
	structure.ident = proxy_ident.clone();
	let orig_generics = structure.generics.clone();
	let (impl_generics, ty_generics, where_clause) = orig_generics.split_for_impl();
	structure.generics = Generics::default();

	quote! {
		const _: () = {
			use ::oro_ser2mem::_detail::Serializable;

			#structure

			#[automatically_derived]
			unsafe impl #impl_generics ::oro_ser2mem::_detail::Proxied for #orig_ident #ty_generics #where_clause {
				type Proxy = #proxy_ident;
			}

			#[automatically_derived]
			unsafe impl #impl_generics ::oro_ser2mem::Serialize for #orig_ident #ty_generics #where_clause {
				unsafe fn serialize<A>(self, alloc: &mut A) where A: ::oro_ser2mem::Allocator {
					const layout: ::core::alloc::Layout = ::core::alloc::Layout::new::<#proxy_ident>();
					alloc.align(layout.align() as u64);
					let base = alloc.position();
					alloc.allocate(layout.size() as u64);
					self.serialize_to(base as *mut #proxy_ident, alloc);
				}
			}

			#[automatically_derived]
			unsafe impl #impl_generics ::oro_ser2mem::_detail::Serializable for #orig_ident #ty_generics #where_clause {
				type Target = #proxy_ident;

				unsafe fn serialize_to<A>(self, to: *mut #proxy_ident, alloc: &mut A) where A: ::oro_ser2mem::Allocator {
					const layout: ::core::alloc::Layout = ::core::alloc::Layout::new::<#proxy_ident>();
					let target = &mut *to;
					alloc.allocate(layout.size() as u64);
					#field_writes
				}
			}
		};
	}
	.into()
}

const ALLOWED_ENUM_REPRS: [&str; 8] = ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"];

fn derive_enum(structure: ItemEnum) -> proc_macro::TokenStream {
	'find_repr: {
		// TODO: Make sure enum is marked non-exhaustive.
		// TODO: This is because C-interop is possible with
		// TODO: ser2mem structures and in C/C++ it is valid
		// TODO: to cast any value to an enum type (whereas it's
		// TODO: not possible in Rust). Thus we need to make sure
		// TODO: usages of the enum in the Kernel account for this.

		for attr in &structure.attrs {
			if let Meta::List(ref maybe_repr) = &attr.meta {
				if maybe_repr.path.leading_colon.is_none()
					&& maybe_repr.path.segments.len() == 1
					&& maybe_repr.path.segments.first().unwrap().ident == "repr"
					&& !maybe_repr.tokens.is_empty()
					&& {
						let first_token = maybe_repr.tokens.clone().into_iter().next().unwrap();
						match first_token {
							TokenTree::Ident(i) => ALLOWED_ENUM_REPRS.iter().any(|&o| i == o),
							_ => false,
						}
					} {
					break 'find_repr;
				}
			}
		}

		// TODO: Enforce that enums are not generic. Should also be enforced with
		// TODO: the below check but providing an extra compiler error can't hurt.

		// TODO: Enforce that all enum variants are fieldless.
		// TODO: Rust has a well-defined mechanism for this, even with
		// TODO: `repr(c)`, but we don't want to support it as it comes
		// TODO: with a lot of extra complexity in both ser2mem as well as
		// TODO: the C implementation, which is neither useful for ser2mem
		// TODO: nor the kernel. Thus, we want to enforce they're not used.

		return Error::new(
			structure.span(),
			"ser2mem enums must be annotated as #[repr(u*/i*)]",
		)
		.into_compile_error()
		.into();
	}

	// TODO: make sure enum is marked

	let ident = structure.ident;

	quote! {
		#[automatically_derived]
		unsafe impl ::oro_ser2mem::_detail::Serializable for #ident {
			type Target = Self;

			#[inline(always)]
			unsafe fn serialize_to<A>(self, to: *mut Self, _alloc: &mut A)
			where
				A: ::oro_ser2mem::Allocator,
			{
				*to = self;
			}
		}
	}
	.into()
}

#[proc_macro_derive(Ser2Mem)]
pub fn derive_ser2mem_structure(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	match parse_macro_input!(item as StructOrEnum) {
		StructOrEnum::Struct(s) => derive_struct(s),
		StructOrEnum::Enum(s) => derive_enum(s),
	}
}
