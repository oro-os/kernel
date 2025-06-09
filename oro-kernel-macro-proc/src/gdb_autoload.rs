//! Core functionality for the `gdb_autoload_x!()` proc macro(s).

use proc_macro2::Span;
use quote::quote;
use syn::{Error, Ident, Lit, Token, punctuated::Punctuated};

#[expect(clippy::missing_docs_in_private_items)]
pub fn gdb_autoload_inline(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let Some(tok) = input.clone().into_iter().next() else {
		return Error::new(Span::call_site(), "expected a string literal")
			.to_compile_error()
			.into();
	};
	let span = tok.span();
	let byte_span = span.byte_range();
	let source_path = span.source_file().path();

	let path_lit = syn::parse_macro_input!(input as syn::LitStr);

	let rel_path = path_lit.value();
	let rel_path = std::path::Path::new(&rel_path);

	// Resolve the path relative to the script that includes it
	let Some(source_dir) = source_path.parent() else {
		return Error::new(Span::call_site(), "failed to resolve source file directory")
			.to_compile_error()
			.into();
	};

	let manifest_path = source_dir.join(rel_path);

	let mut script = match std::fs::read(manifest_path) {
		Ok(b) => b,
		Err(e) => {
			return Error::new(
				Span::call_site(),
				format!("failed to load script: {e}: {rel_path:?}"),
			)
			.to_compile_error()
			.into();
		}
	};

	// Tell GDB this is an inline script
	script.splice(0..0, "gdb.inlined-script\n".bytes());

	// Prefix the `0x04` byte to indicate it's a literal python script
	script.insert(0, 0x04);

	// Postfix a null byte to indicate the end of the script
	script.push(0x00);

	// Create a byte array literal from the script
	let mut script_lit = Punctuated::<Lit, Token![,]>::new();
	let script_len = script.len();
	for b in script {
		script_lit.push(Lit::Int(syn::LitInt::new(
			&b.to_string(),
			Span::call_site(),
		)));
	}

	let script_ident = Ident::new(
		&format!("_SCRIPT_{}_{}", byte_span.start, byte_span.end),
		Span::call_site(),
	);

	let expanded = quote! {
			#[used]
			#[unsafe(link_section = ".debug_gdb_scripts")]
			static #script_ident: [u8; #script_len] = {
					// This is unused, but creates a build-time dependency
					// on the file.
					include_bytes!(#path_lit);
					[ #script_lit ]
			};
	};

	expanded.into()
}
