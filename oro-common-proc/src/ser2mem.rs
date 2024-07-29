//! Implements the `#[derive(Ser2Mem)]` derive proc macro.
//!
//! This macro generates implementations of the serializer
//! used to write Rust structures directly to another point
//! in memory, linearly, with proper alignment and padding
//! in order to be read by the kernel via a simple virtual
//! memory mapping and pointer cast.
//!
//! It's use is exclusively for the boot protocol and should
//! not be used for anything else.
#![allow(clippy::missing_docs_in_private_items)]

use crate::syn_ex::{ReprArgs, ReprAttribute};
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Ident};

pub fn derive_ser2mem(input: proc_macro::TokenStream) -> syn::Result<proc_macro::TokenStream> {
	let input: DeriveInput = syn::parse(input)?;

	let repr_args = match input.repr_args() {
		Ok(ra) => ra,
		Err(e) => {
			return Err(Error::new_spanned(
				input,
				format!(
					"Ser2Mem requires a #[repr(...)] attribute, but one could not be parsed: {e}",
				),
			));
		}
	};

	if repr_args.packed {
		return Err(Error::new(
			repr_args.span,
			"Ser2Mem does not support packed structs",
		));
	}

	if !input.generics.params.is_empty() {
		return Err(Error::new_spanned(
			input.generics.params,
			"Ser2Mem does not support generic types",
		));
	}

	match input.data {
		Data::Enum(d) => derive_ser2mem_enum(&input.ident, &d, &repr_args),
		Data::Struct(d) => derive_ser2mem_struct(&input.ident, &d, &repr_args),
		Data::Union(_) => {
			Err(Error::new_spanned(
				input,
				"Ser2Mem cannot be derived for unions",
			))
		}
	}
}

fn derive_ser2mem_enum(
	ident: &Ident,
	data: &DataEnum,
	repr_args: &ReprArgs,
) -> syn::Result<proc_macro::TokenStream> {
	// SAFETY(qix-): There isn't any *technical* reason in terms of implementing the serialization
	// SAFETY(qix-): why these checks are needed but it keeps the boot protocol types consistent,
	// SAFETY(qix-): especially over time.
	if let Some(abi) = &repr_args.abi {
		match ["u8", "u16", "u32", "u64"]
			.into_iter()
			.find_map(|x| if abi == x { Some(abi.clone()) } else { None })
		{
			Some(abi) => abi,
			None => {
				return Err(Error::new_spanned(
					abi,
					"Ser2Mem enums require a #[repr(u8|u16|u32|u64)] attribute, but an \
					 unsupported base type was given",
				));
			}
		}
	} else {
		return Err(Error::new(
			repr_args.span,
			"Ser2Mem enums require a #[repr(u8|u16|u32|u64)] attribute with a supported base \
			 type, but no ABI was given",
		));
	};

	for variant in &data.variants {
		if !variant.fields.is_empty() {
			return Err(Error::new_spanned(
				variant,
				"Ser2Mem can only be derived for unit variant enums",
			));
		}

		if variant.discriminant.is_none() {
			return Err(Error::new_spanned(
				variant,
				"Ser2Mem can only be derived for enums with explicit discriminants",
			));
		}
	}

	Ok(quote! {
		const _: () = {
			#[automatically_derived]
			unsafe impl crate::ser2mem::Serialize for #ident {
				type Output = Self;

				#[inline]
				unsafe fn serialize<S: crate::ser2mem::Serializer>(self, _s: &mut S) -> Result<Self::Output, S::Error> {
					Ok(self)
				}
			}
		};
	}.into())
}

#[allow(clippy::too_many_lines)]
fn derive_ser2mem_struct(
	ident: &Ident,
	data: &DataStruct,
	repr_args: &ReprArgs,
) -> syn::Result<proc_macro::TokenStream> {
	if let Some(abi) = &repr_args.abi {
		if abi != "C" {
			return Err(Error::new_spanned(
				abi,
				"Ser2Mem structs require a #[repr(C, ...)] ABI specifier, but a different ABI was \
				 given",
			));
		}
	} else {
		return Err(Error::new(
			repr_args.span,
			"Ser2Mem structs require a #[repr(...)] attribute with a C ABI specifier, but no ABI \
			 was given",
		));
	}

	let proxy_ident = Ident::new(&format!("{ident}Proxy"), ident.span());
	let mut needs_lifetime = false;

	let mut proxy_fields = vec![];
	let mut field_serializations = vec![];
	let mut field_writes = vec![];

	for field in &data.fields {
		let ident = field.ident.as_ref().ok_or_else(|| {
			Error::new_spanned(field, "Ser2Mem can only be derived for named fields")
		})?;

		let ty = &field.ty;

		// If the type is a static const reference to an array of T...
		let proxy_ty = if let syn::Type::Reference(syn::TypeReference {
			elem,
			lifetime,
			mutability,
			..
		}) = ty
		{
			if mutability.is_some() {
				return Err(Error::new_spanned(
					ty,
					"Ser2Mem reference fields must be immutable",
				));
			}

			if lifetime.is_none() || matches!(lifetime, Some(lt) if lt.ident != "static") {
				return Err(Error::new_spanned(
					ty,
					"Ser2Mem reference fields must be 'static",
				));
			}

			if let syn::Type::Slice(syn::TypeSlice { elem, .. }) = (*elem).as_ref() {
				// We need to pass in a lifetime since we use `DynIter` in the proxy struct.
				needs_lifetime = true;
				quote! {
					crate::ser2mem::DynIter<'iter, <#elem as crate::ser2mem::Proxy>::Proxy<'iter>>
				}
			} else {
				quote! {
					#elem
				}
			}
		} else {
			quote! {
				#ty
			}
		};

		let temp_ident = Ident::new(&format!("{ident}__out"), ident.span());

		proxy_fields.push(quote! {
			pub #ident: #proxy_ty,
		});

		field_serializations.push(quote! {
			let #temp_ident = self.#ident.serialize(s)?;
		});

		field_writes.push(quote! {
			#ident: #temp_ident,
		});
	}

	let lt = if needs_lifetime {
		quote! { <'iter> }
	} else {
		quote! {}
	};

	Ok(quote! {
		const _: () = {
			#[automatically_derived]
			pub struct #proxy_ident #lt {
				#(#proxy_fields)*
			}

			#[automatically_derived]
			impl crate::ser2mem::Proxy for #ident {
				type Proxy<'iter> = #proxy_ident #lt;
			}

			#[automatically_derived]
			unsafe impl #lt crate::ser2mem::Serialize for #proxy_ident #lt {
				type Output = &'static #ident;

				#[allow(non_snake_case)]
				unsafe fn serialize<S: crate::ser2mem::Serializer>(self, s: &mut S) -> Result<Self::Output, S::Error> {
					let layout = ::core::alloc::Layout::new::<#ident>();

					#(#field_serializations)*

					let base = s.align_to(layout.align())?;

					s.write(#ident {
						#(#field_writes)*
					})?;

					Ok(&*base.cast())
				}
			}
		};
	}
	.into())
}
