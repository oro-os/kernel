//! Core functionality for the `#[derive(EnumIterator)]` proc macro.

use quote::quote;

#[allow(clippy::missing_docs_in_private_items)]
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
