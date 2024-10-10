//! Implements the global resource macros - namely the `#[oro_global_*]` attribute
//! and the `oro_global_getter!` macro.
#![expect(clippy::missing_docs_in_private_items)]

struct GlobalResourceGetters {
	punctuated: syn::punctuated::Punctuated<GlobalResourceGetter, syn::token::Comma>,
}

struct GlobalResourceGetter {
	vis:          syn::Visibility,
	ident:        syn::Ident,
	_arrow_token: syn::token::RArrow,
	ty:           syn::Type,
}

impl syn::parse::Parse for GlobalResourceGetters {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let punctuated = input.parse_terminated(GlobalResourceGetter::parse, syn::Token![,])?;
		Ok(Self { punctuated })
	}
}

impl syn::parse::Parse for GlobalResourceGetter {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Self {
			vis:          input.parse()?,
			ident:        input.parse()?,
			_arrow_token: input.parse()?,
			ty:           input.parse()?,
		})
	}
}

pub fn global_getter(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let getters = syn::parse_macro_input!(input as GlobalResourceGetters);

	let mut tokens = proc_macro2::TokenStream::new();

	for getter in getters.punctuated.iter() {
		let ident = &getter.ident;
		let extern_ident = syn::Ident::new(&format!("__oro_global_getter_{ident}"), ident.span());
		let ty = &getter.ty;
		let vis = &getter.vis;
		tokens.extend(quote::quote! {
			#[allow(clippy::missing_docs_in_private_items, clippy::inline_always)]
			#[inline(always)]
			#vis fn #ident() -> &'static dyn #ty {
				extern "C" {
					fn #extern_ident() -> &'static dyn #ty;
				}

				unsafe {
					#extern_ident()
				}
			}
		});
	}

	tokens.into()
}

pub fn mark_global_resource(
	args: proc_macro::TokenStream,
	input: proc_macro::TokenStream,
	name: &str,
	ty: syn::Type,
) -> proc_macro::TokenStream {
	// Make sure there are no arguments
	if !args.is_empty() {
		return syn::Error::new(
			proc_macro2::Span::call_site(),
			"this macro does not take any arguments",
		)
		.to_compile_error()
		.into();
	}

	let input = syn::parse_macro_input!(input as syn::ItemStatic);

	let ident = &input.ident;
	let given_ty = &input.ty;
	let extern_ident = syn::Ident::new(
		&format!("__oro_global_getter_{name}"),
		proc_macro2::Span::call_site(),
	);

	quote::quote! {
		#input

		const _: () = {
			#[no_mangle]
			extern "C" fn #extern_ident() -> &'static dyn #ty {
				#[allow(static_mut_refs)]
				unsafe { &#ident }
			}

			// Make sure it adheres to the expected type.
			#[allow(clippy::missing_docs_in_private_items)]
			#[doc(hidden)]
			const _: fn() = || {
				fn _assert_type<T: #ty>() {}
				_assert_type::<#given_ty>();
			};
		};
	}
	.into()
}
