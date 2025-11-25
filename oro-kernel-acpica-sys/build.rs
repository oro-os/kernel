use ::convert_case::{Case, Casing};
use ::quote::ToTokens;

fn main() {
	let out_dir = std::env::var("OUT_DIR").unwrap();
	let dest_path = std::path::Path::new(&out_dir).join("bindings.rs");

	// Bindgen doesn't know how to handle custom Oro targets,
	// so we work around this by unsetting `TARGET` and using
	// the host system's sysroot.
	//
	// In the general case, cross-compilation like this wouldn't
	// work. However, with the way we're using ACPICA, I don't
	// believe this to be an issue.
	unsafe {
		std::env::remove_var("TARGET");
	}

	let bindings = ::bindgen::Builder::default()
		.header("oro-acpica-sys.h")
		.derive_debug(true)
		.default_enum_style(::bindgen::EnumVariation::Rust {
			non_exhaustive: true,
		})
		.clang_arg("-Isrc-acpica/source/include")
		.parse_callbacks(Box::new(::bindgen::CargoCallbacks::new()))
		.ignore_functions()
		.ignore_methods()
		.use_core()
		.disable_nested_struct_naming()
		.rust_target(::bindgen::RustTarget::nightly())
		.size_t_is_usize(true)
		.translate_enum_integer_types(true)
		.detect_include_paths(true)
		.raw_line("#[allow(clippy::doc_markdown)]");

	#[cfg(target_arch = "x86_64")]
	let bindings = bindings.clang_arg("-D__x86_64__");
	#[cfg(target_arch = "aarch64")]
	let bindings = bindings.clang_arg("-D__aarch64__");
	#[cfg(target_arch = "riscv64")]
	let bindings = bindings.clang_arg("-D__risc");
	#[cfg(target_arch = "powerpc64")]
	let bindings = bindings.clang_arg("-D__PPC64__");
	#[cfg(target_arch = "s390x")]
	let bindings = bindings.clang_arg("-D__s390x__");
	#[cfg(target_arch = "loongarch64")]
	let bindings = bindings.clang_arg("-D__loongarch__");

	println!("bindgen args: {:?}", bindings.command_line_flags());

	let bindings = bindings.generate().expect("unable to generate bindings");

	let macro_dest_path = std::path::Path::new(&out_dir).join("tablegen_macro.rs");

	let mut buf = Vec::with_capacity(1024 * 1024 * 10);
	bindings
		.write(Box::from(&mut buf))
		.expect("unable to write bindings to string");

	let src = String::from_utf8(buf).expect("bindings are not utf-8");

	let mut bindings = ::syn::parse_file(&src).expect("unable to parse bindings");

	wrap_table_types(&mut bindings).expect("unable to wrap table types");
	let macr = generate_tablegen_macro(&bindings).expect("unable to generate tablegen macro");
	std::fs::write(
		macro_dest_path,
		macr.to_token_stream().to_string().as_bytes(),
	)
	.expect("unable to write tablegen macro");

	std::fs::write(dest_path, bindings.to_token_stream().to_string().as_bytes())
		.expect("unable to write wrapped type bindings");
}

fn generate_tablegen_macro(bindings: &::syn::File) -> ::syn::Result<impl ::quote::ToTokens> {
	let mut strukts = std::collections::HashMap::new();

	for item in &bindings.items {
		if let ::syn::Item::Struct(strukt) = item {
			let struct_ident = strukt.ident.to_string();
			let mini_ident = {
				let ident_splits = struct_ident.split('_').collect::<Vec<_>>();
				let &["acpi", "table", ident] = ident_splits.as_slice() else {
					continue;
				};
				ident.to_string()
			};
			strukts.insert(
				mini_ident,
				(
					struct_ident,
					strukt
						.attrs
						.iter()
						.filter(|attr| attr.path().is_ident("doc"))
						.collect::<Vec<_>>(),
				),
			);
		}
	}

	let mut tokens = Vec::new();

	for item in &bindings.items {
		if let ::syn::Item::Const(item) = item {
			let const_ident = item.ident.to_string();
			let mini_ident = {
				let ident_splits = const_ident.split('_').collect::<Vec<_>>();
				let &["ACPI", "SIG", ident] = ident_splits.as_slice() else {
					continue;
				};
				ident.to_string()
			};

			let ident = syn::Ident::new(&mini_ident.to_case(Case::Pascal), item.ident.span());

			let sig_value = &item.ident;
			let sig_ty = &item.ty;

			if let Some((strukt, docs)) = strukts.get(&mini_ident.to_ascii_lowercase()) {
				let strukt = syn::Ident::new(strukt, item.ident.span());

				tokens.push(::quote::quote! {
					#ident => ($crate::#strukt, $crate::#sig_value, ( #sig_ty ), #(#docs)*),
				});
			}
		}
	}

	Ok(::quote::quote! {
		/// Calls the given macro with a list of all discovered ACPI tables.
		///
		/// # Example
		///
		/// ```no_run
		/// use oro_kernel_acpica_sys::acpi_tables;
		///
		/// macro_rules! impl_tables {
		/// 	($($slug:tt => ($strukt:ident, $sig:ident, ( $sigty:ty ), $(#[doc = $doc:literal]),*)),* $(,)?) => {
		/// 		$(println!(
		/// 			"slug={} struct={} sig={} : {}",
		/// 			stringify!($slug),
		/// 			stringify!($strukt),
		/// 			stringify!($sig),
		/// 			stringify!($sigty),
		/// 		);)*
		/// 	};
		/// }
		///
		/// fn main() {
		/// 	acpi_tables!(impl_tables);
		/// }
		/// ```
		///
		/// would print:
		///
		/// ```text
		/// slug=Rsdp struct=acpi_table_rsdp sig=ACPI_SIG_RSDP : &[u8; 9]
		/// slug=Madt struct=acpi_table_madt sig=ACPI_SIG_MADT : &[u8; 5]
		/// slug=Facp struct=acpi_table_facp sig=ACPI_SIG_FACP : &[u8; 5]
		/// ...
		/// ```
		///
		/// The order of emitted items is in no guaranteed
		/// order.
		#[macro_export]
		macro_rules! acpi_tablegen {
			($macro_:tt) => {
				$macro_! {
					#(#tokens)*
				}
			};
		}
	})
}

fn wrap_table_types(bindings: &mut syn::File) -> syn::Result<()> {
	for item in &mut bindings.items {
		if let syn::Item::Struct(strukt) = item {
			// Skip any structures that aren't ACPI tables / the header.
			//
			// NOTE(qix-): I'm tentatively disabling this check because it's
			// NOTE(qix-): becoming apparent that all structs need to be wrapped.
			// NOTE(qix-): If this breaks something we'll have to get a bit
			// NOTE(qix-): more clever about it.
			//
			// let struct_ident = strukt.ident.to_string();
			// if !struct_ident.starts_with("acpi_table_") {
			// 	continue;
			// }

			// For every field in the structure, if it's one of the primitive numeric
			// types (including f32 and f64 but not `bool`) then wrap the `foo: T` (where
			// `T` is the primitive type) with `Le<T>`.
			for field in &mut strukt.fields {
				match field.ty {
					syn::Type::Path(ref mut path) => {
						if path.path.segments.len() != 1 {
							continue;
						}

						if path.qself.is_some() {
							continue;
						}

						let first = path.path.segments.first().unwrap();
						if !first.arguments.is_empty() {
							continue;
						}

						let ident = first.ident.to_string();
						if !matches!(
							ident.as_str(),
							"u8" | "u16"
								| "u32" | "u64" | "u128" | "usize"
								| "i8" | "i16" | "i32" | "i64"
								| "i128" | "isize" | "UINT32"
								| "UINT64" | "UINT8" | "UINT16"
								| "UINT128" | "UINTPTR" | "INT32"
								| "INT64" | "INT8" | "INT16"
								| "INT128"
						) {
							continue;
						}
					}
					_ => {
						continue;
					}
				}

				// It's a numeric primitive; wrap it in `Le<T>`.
				let ty = &field.ty;
				field.ty = syn::parse_quote!(::oro_kernel_type::Le<#ty>);
			}
		}
	}

	Ok(())
}
