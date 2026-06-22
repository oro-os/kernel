use proc_macro::TokenStream;
use quote::quote;
use syn::{self, parse_quote, Arm, Data};

#[proc_macro_derive(Name)]
pub fn name(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse_macro_input!(input);
    let ident = &ast.ident;
    let gen = quote! {
        impl derive_name::Name for #ident {
            fn name() -> &'static str {
                stringify!(#ident)
            }
        }
    };
    gen.into()
}

#[proc_macro_derive(VariantName)]
pub fn variant(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse_macro_input!(input);

    if let Data::Enum(r#enum) = &ast.data {
        let ident = &ast.ident;
        let mut match_arms = Vec::<Arm>::with_capacity(r#enum.variants.len());

        for variant in r#enum.variants.iter() {
            let variant_ident = &variant.ident;
            let match_pattern = match &variant.fields {
                syn::Fields::Named(_) => {
                    quote!( Self::#variant_ident {..} )
                }
                syn::Fields::Unnamed(_) => {
                    quote!( Self::#variant_ident (..) )
                }
                syn::Fields::Unit => quote!( Self::#variant_ident ),
            };

            match_arms.push(parse_quote! {
                #match_pattern => stringify!(#variant_ident)
            });
        }
        let gen = quote! {
            impl derive_name::VariantName for #ident {
                fn variant_name(&self) -> &'static str {
                    match self {
                        #(#match_arms),*
                    }
                }
            }
        };
        gen.into()
    } else {
        quote!(
            compile_error!("Can only implement 'VariantName' on a enum");
        )
        .into()
    }
}
