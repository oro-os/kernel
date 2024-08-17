#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashSet;
use syn::spanned::Spanned;

pub fn vla(
	attr: proc_macro::TokenStream,
	input: proc_macro::TokenStream,
) -> syn::Result<proc_macro::TokenStream> {
	// `#[vla]` takes only a single optional argument: `allow_missing`.
	let mut allow_missing = false;
	if !attr.is_empty() {
		let meta = syn::parse::<syn::Meta>(attr)?;
		if let syn::Meta::Path(path) = meta {
			if path.is_ident("allow_missing") {
				allow_missing = true;
			} else {
				return Err(syn::Error::new_spanned(path, "unknown attribute"));
			}
		} else {
			return Err(syn::Error::new_spanned(meta, "expected path"));
		}
	}

	let mut struct_item: syn::ItemStruct = syn::parse(input)?;

	let struct_name = &struct_item.ident;
	let mut fields = struct_item.fields.iter_mut().collect::<Vec<_>>();
	let Some(last_field) = fields.pop() else {
		return Err(syn::Error::new_spanned(
			struct_item,
			"#[vla] struct must have at least one field",
		));
	};

	let mut field_names = HashSet::new();

	for field in fields {
		if field.attrs.iter().any(|attr| attr.path().is_ident("vla")) {
			return Err(syn::Error::new_spanned(
				field,
				"field with #[vla] attribute must be the last field",
			));
		}

		if let Some(ident) = &field.ident {
			field_names.insert(ident.clone());
		}
	}

	let vla_len_field = match last_field
		.attrs
		.iter()
		.find(|attr| attr.path().is_ident("vla"))
	{
		Some(last_vla) => {
			if let syn::Meta::List(syn::MetaList { tokens, .. }) = &last_vla.meta {
				let mut iter = tokens.clone().into_iter();
				let Some(first) = iter.next() else {
					return Err(syn::Error::new_spanned(
						last_vla,
						"expected #[vla(...)] to specify the count field",
					));
				};

				if let proc_macro2::TokenTree::Ident(first) = first {
					Some(first)
				} else {
					return Err(syn::Error::new_spanned(
						first,
						"expected identifier for #[vla(...)] count field",
					));
				}
			} else {
				return Err(syn::Error::new_spanned(
					last_vla,
					"expected #[vla(...)] to specify the count field",
				));
			}
		}
		None => None,
	};

	let Some(vla_len_field) = vla_len_field else {
		return if allow_missing {
			// Just return the struct. There's nothing to do.
			Ok(quote::quote!(#struct_item).into())
		} else {
			Err(syn::Error::new_spanned(
				last_field,
				"last field in struct must have #[vla] attribute",
			))
		};
	};

	last_field.attrs.retain(|attr| !attr.path().is_ident("vla"));

	let concrete_type = if let syn::Type::Array(last_array) = &mut last_field.ty {
		last_array.elem.clone()
	} else {
		return Err(syn::Error::new_spanned(
			last_field,
			"last field in #[vla] struct must be an array",
		));
	};

	if !field_names.contains(&vla_len_field) {
		return Err(syn::Error::new_spanned(
			&vla_len_field,
			format!(
				"#[vla] field array length must be a struct field (`{}` not found in struct)",
				vla_len_field
			),
		));
	}

	let last_field_name = last_field
		.ident
		.clone()
		.unwrap_or_else(|| syn::Ident::new("__VLA", last_field.span()));

	last_field.ident = Some(last_field_name.clone());
	let last_field_mut_name =
		syn::Ident::new(&format!("{}_mut", last_field_name), last_field_name.span());

	let vis = last_field.vis.clone();
	last_field.vis = syn::Visibility::Inherited;

	let (gen_impl, gen_params, gen_where) = struct_item.generics.split_for_impl();

	Ok(quote::quote! {
		#struct_item

		#[automatically_derived]
		impl #gen_impl #struct_name #gen_params #gen_where {
			/// Returns an immutable slice of the VLA field with
			/// a count of `self.#vla_len_field`.
			///
			/// # Safety
			/// Caller must ensure that `self.#vla_len_field` is a valid
			/// count of the array.
			#vis unsafe fn #last_field_name(&self) -> &[#concrete_type] {
				let len = usize::from(self.#vla_len_field);
				let start = (self.#last_field_name).as_ptr();
				core::slice::from_raw_parts(start, len)
			}

			/// Returns a mutable slice of the VLA field with
			/// a count of `self.#vla_len_field`.
			///
			/// # Safety
			/// Caller must ensure that `self.#vla_len_field` is a valid
			/// count of the array.
			#vis unsafe fn #last_field_mut_name(&mut self) -> &mut [#concrete_type] {
				let len = usize::from(self.#vla_len_field);
				let start = (self.#last_field_name).as_mut_ptr();
				core::slice::from_raw_parts_mut(start, len)
			}
		}
	}
	.into())
}
