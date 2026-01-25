use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

pub fn effect(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
	let fn_item: syn::ItemFn = syn::parse2(input)?;
	let fn_name = fn_item.sig.ident.to_string();

	let attrs = &fn_item.attrs;
	let vis = &fn_item.vis;
	let sig = &fn_item.sig;
	let block = &fn_item.block;

	Ok(quote! {
		#(#attrs)*
		#vis #sig {
			::orok_test::annotate_effect_fn! {
				start @ #fn_name => {
					#attr
				}
			}

			let return_result__effect__ = #block;

			::orok_test::annotate_effect_fn! {
				end @ #fn_name => {
					#attr
				}
			}

			return_result__effect__
		}
	}
	.into())
}
