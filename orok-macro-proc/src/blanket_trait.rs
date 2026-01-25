use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

pub fn blanket_trait(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
	if !attr.is_empty() {
		return Err(syn::Error::new_spanned(
			attr,
			"#[blanket_trait] does not accept any attributes",
		));
	}

	let item_trait: syn::ItemTrait = syn::parse2(input.clone())?;
	let trait_ident = &item_trait.ident;
	let generics = &item_trait.generics;
	if !generics.params.is_empty() {
		return Err(syn::Error::new_spanned(
			generics,
			"#[blanket_trait] does not support traits with generic parameters",
		));
	}

	let supertraits = &item_trait.supertraits;

	Ok(quote! {
		#input

		impl<T> #trait_ident for T where T: #supertraits {}
	})
}
