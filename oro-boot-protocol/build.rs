#![expect(missing_docs, clippy::missing_docs_in_private_items)]

use std::{io::Write, path::PathBuf, process::Command};

use convert_case::{Case, Casing};

const BLACKLIST_MODS: [&str; 1] = ["macros"];
const VALID_REPR_TYPES: [&str; 4] = ["u8", "u16", "u32", "u64"]; // NOTE(qix-): usize explicitly excluded

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! warn {
	($($arg:tt)*) => {
		println!("cargo:warning=oro-boot.h: {}", format_args!($($arg)*));
	};
}

#[expect(clippy::too_many_lines)]
fn main() {
	println!("cargo::rustc-check-cfg=cfg(oro_build_protocol_header)");
	println!("cargo::rerun-if-env-changed=ORO_BUILD_PROTOCOL_HEADER");

	if std::env::var("ORO_BUILD_PROTOCOL_HEADER")
		.map(|v| v != "1")
		.unwrap_or(true)
	{
		return;
	}

	// SAFETY: This is safe to call in a single-threaded environment (we are).
	unsafe {
		std::env::remove_var("ORO_BUILD_PROTOCOL_HEADER");
	}

	let tmpdir = std::env::temp_dir().join("oro-boot-protocol-header");
	let tmpdir = tmpdir.to_str().expect("failed to get temp dir");
	std::fs::create_dir(tmpdir)
		.or_else(|e| {
			if e.kind() == std::io::ErrorKind::AlreadyExists {
				Ok(())
			} else {
				Err(e)
			}
		})
		.unwrap_or_else(|_| panic!("failed to create temp dir: {tmpdir}"));

	let mut command = Command::new(env!("CARGO"));
	command.stdout(std::process::Stdio::piped());
	command.env_remove("ORO_BUILD_PROTOCOL_HEADER");
	command.args([
		"rustc",
		"--lib",
		"-p",
		"oro-boot-protocol",
		// TODO(qix-): Support offline somehow.
		// "--offline",
		"--",
		"--cfg",
		"oro_build_protocol_header",
		"-Zunpretty=expanded",
	]);
	command.env("CARGO_TARGET_DIR", tmpdir);
	let result = command
		.output()
		.expect("failed to expand oro-boot-protocol");

	if !result.status.success() {
		std::io::stderr().write_all(&result.stderr).unwrap();
		panic!("failed to expand oro-boot-protocol");
	}

	let output = String::from_utf8(result.stdout)
		.expect("failed to convert expansion output to utf-8 string");

	let expanded = syn::parse_file(&output).expect("failed to parse expanded.rs");

	let output_file = {
		let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
		let profile = std::env::var("PROFILE").expect("PROFILE not set");
		let mut path = PathBuf::from(out_dir)
			.canonicalize()
			.expect("failed to canonicalize OUT_DIR");
		while let Some(filename) = path.file_name() {
			if *filename == *profile {
				path.pop();
				break;
			}
			path.pop();
		}

		path.join("oro-boot.h")
	};

	let mut out = std::fs::File::create(&output_file).unwrap_or_else(|_| {
		panic!("failed to create Oro boot protocol header file: {output_file:?}")
	});

	writeln!(out, "#ifndef ORO_BOOT__H").unwrap();
	writeln!(out, "#define ORO_BOOT__H").unwrap();
	writeln!(out, "#pragma once\n").unwrap();
	writeln!(out, "/*\n\tTHIS IS AN AUTOGENERATED FILE. DO NOT EDIT.\n").unwrap();
	writeln!(out).unwrap();
	writeln!(out, "\t⠀⠀⠀⠀⠀⠀⠀⣀⣤⣤⣤⣤⣤⣀⠔⠂⠉⠉⠑⡄").unwrap();
	writeln!(out, "\t⠀⠀⠀⠀⢠⣴⠟⠋⠉⠀⠀⠀⠉⠙⠻⣦⣀⣤⣤⣇").unwrap();
	writeln!(out, "\t⠀⠀⠀⣰⡟⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⠟⠉⠉⢻⣧⠀⠀⠀⠀⠀ORO OPERATING SYSTEM").unwrap();
	writeln!(
		out,
		"\t⠀⠀⢰⡿⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢿⣆⡀⢀⣸⡟⠀⠀⠀⠀⠀boot protocol header for C/C++"
	)
	.unwrap();
	writeln!(out, "\t⠀⠀⢸⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠛⢻⡟⠋⠀⠀⠀⠀⠀⠀github.com/oro-os/kernel").unwrap();
	writeln!(
		out,
		"\t⠀⠀⠸⣷⠀⠀⠀⠀⠀⠀⠀⠀⠀⡠⠊⠀⠀⣿⠃⠀⠀⠀⠀⠀⠀⠀copyright (c) 2024, Josh Junon"
	)
	.unwrap();
	writeln!(
		out,
		"\t⠀⠀⡐⠹⣧⡀⠀⠀⠀⠀⠀⡠⠊⠀⠀⢀⣾⠏⠀⠀⠀⠀⠀⠀⠀⠀MPL-2.0 License (see below)"
	)
	.unwrap();
	writeln!(out, "\t⠀⢰⠀⠀⠘⠻⣦⣄⣀⡔⠊⠀⣀⣠⣴⠟⠁").unwrap();
	writeln!(out, "\t⠀⠘⢄⣀⣀⠠⠔⠉⠛⠛⠛⠛⠛⠉").unwrap();
	writeln!(out, "\n\n").unwrap();
	writeln!(out, "\tThis header is generated from the Rust").unwrap();
	writeln!(out, "\tsource code of the `oro-boot-protocol`").unwrap();
	writeln!(out, "\tcrate.\n").unwrap();
	writeln!(out, "\tFor all documentation on how to use").unwrap();
	writeln!(out, "\tthis header, refer to").unwrap();
	writeln!(out, "\thttps://github.com/oro-os/kernel.\n").unwrap();
	writeln!(out, "\tThe Oro Operating System kernel").unwrap();
	writeln!(out, "\tproject is licensed under the Mozilla").unwrap();
	writeln!(out, "\tPublic License 2.0, with the exception").unwrap();
	writeln!(out, "\tof this header, which is dual-licensed").unwrap();
	writeln!(out, "\tunder the MIT License and the Creative").unwrap();
	writeln!(out, "\tCommons 0 (CC0) license. You may choose").unwrap();
	writeln!(out, "\tto use this header under either license,").unwrap();
	writeln!(out, "\tat your discretion.").unwrap();
	writeln!(out, "*/\n").unwrap();
	writeln!(out, "#ifdef __cplusplus").unwrap();
	writeln!(out, "#include <cstdint>").unwrap();
	writeln!(out, "#define ORO_BOOT_ENUM(ty, name) name").unwrap();
	writeln!(out, "namespace oro_boot {{").unwrap();
	writeln!(out, "#else").unwrap();
	writeln!(out, "#include <stdint.h>").unwrap();
	writeln!(
		out,
		"#define ORO_BOOT_ENUM(ty, name) ORO_BOOT_##ty##_##name"
	)
	.unwrap();
	writeln!(out, "#endif\n").unwrap();
	writeln!(out, "#ifndef ORO_BOOT_ALIGN").unwrap();
	writeln!(out, "#ifdef _MSC_VER").unwrap();
	writeln!(
		out,
		"#define ORO_BOOT_ALIGN(n, ...) __declspec(align(n)) __VA_ARGS__"
	)
	.unwrap();
	writeln!(out, "#else").unwrap();
	writeln!(
		out,
		"#define ORO_BOOT_ALIGN(n, ...) __VA_ARGS__ __attribute__((aligned(n)))"
	)
	.unwrap();
	writeln!(out, "#endif").unwrap();
	writeln!(out, "#endif\n\n").unwrap();

	// NOTE(qix-): We could support packing but since it's not yet needed
	// NOTE(qix-): it's been omitted for now. If it ever becomes necessary,
	// NOTE(qix-): uncomment all of the pack-related blocks. Note that the
	// NOTE(qix-): `packed` attribute would have to be checked and the macro
	// NOTE(qix-): would have to be used in the struct definitions.
	// writeln!(out, "#ifndef ORO_BOOT_PACKED").unwrap();
	// writeln!(out, "#ifdef _MSC_VER").unwrap();
	// writeln!(out, "#define ORO_BOOT_PACKED(...) __pragma(pack(push, 1)) __VA_ARGS__ __pragma(pack(pop))").unwrap();
	// writeln!(out, "#else").unwrap();
	// writeln!(out, "#define ORO_BOOT_PACKED(...) __VA_ARGS__ __attribute__((packed))").unwrap();
	// writeln!(out, "#endif").unwrap();
	// writeln!(out, "#endif\n").unwrap();

	process_tags(&expanded.items, &mut out)
		.expect("failed to process oro-boot-protocol header tags");
	process_mod(&expanded.items, &mut out).expect("failed to process oro-boot-protocol header");

	writeln!(out, "#ifdef __cplusplus").unwrap();
	writeln!(out, "}}").unwrap();
	writeln!(out, "#endif").unwrap();
	writeln!(out, "#endif").unwrap();

	out.flush().unwrap();

	println!("cargo:rerun-if-changed={}", output_file.to_str().unwrap());
}

fn process_tags<W: Write>(items: &[syn::Item], w: &mut W) -> Result<()> {
	for item in items {
		if let syn::Item::Type(item) = item {
			if let Some(doc) = extract_doc_comment(&item.attrs, 0) {
				writeln!(w, "{doc}")?;
			} else {
				warn!("missing documentation for type: {}", item.ident);
			}

			let ident = to_hungarian(&item.ident.to_string());
			let (base, arr) = type_to_ctype(&item.ty)
				.map_err(|e| format!("failed to process type {}: {e}", item.ident))?;

			writeln!(w, "typedef {base} {ident}{arr};\n")?;
		}
	}

	for item in items {
		if let syn::Item::Impl(item) = item {
			if let Some((None, trt, _)) = &item.trait_ {
				let seg = trt.segments.last().unwrap();
				if seg.ident == "RequestTag" {
					let target = &item.self_ty;
					let syn::Type::Path(ref target) = **target else {
						return Err(format!(
							"expected a path type for RequestTag impl, found {target:?}"
						)
						.into());
					};

					let target_ident = target.path.segments.last().unwrap().ident.to_string();

					let Some(tag_item) = item.items.iter().find(|item| {
						if let syn::ImplItem::Const(item) = item {
							item.ident == "TAG"
						} else {
							false
						}
					}) else {
						return Err(format!(
							"missing TAG constant for RequestTag impl: {target_ident}"
						)
						.into());
					};

					let syn::ImplItem::Const(syn::ImplItemConst { expr, .. }) = &tag_item else {
						return Err(format!(
							"expected a constant for RequestTag impl, found {tag_item:?}"
						)
						.into());
					};

					let syn::Expr::Lit(syn::ExprLit { lit, .. }) = expr else {
						return Err(format!(
							"expected a literal for RequestTag impl, found {expr:?}"
						)
						.into());
					};

					let syn::Lit::ByteStr(bs) = lit else {
						return Err(format!(
							"expected a byte string for RequestTag impl, found {lit:?}"
						)
						.into());
					};

					let target_ident = target_ident.to_case(Case::ScreamingSnake);

					write!(w, "#define ORO_BOOT_REQ_{target_ident}_ID (*(oro_tag_t*)\"")?;
					w.write_all(&bs.value())?;
					writeln!(w, "\")")?;
				}
			}
		}
	}

	writeln!(w, "\n")?;

	Ok(())
}

#[expect(clippy::too_many_lines)]
fn process_mod<W: Write>(items: &[syn::Item], w: &mut W) -> Result<()> {
	// Collect them so we can sort them
	let mut items: Vec<_> = items.iter().collect();
	items.sort_by_key(|item| {
		match item {
			syn::Item::Const(_) => 0,
			syn::Item::Enum(_) => 1,
			syn::Item::Mod(_) => 2,
			_ => 3,
		}
	});

	for item in items {
		match item {
			syn::Item::Mod(item) => {
				if !BLACKLIST_MODS.contains(&item.ident.to_string().as_str()) {
					if let Some(items) = &item.content {
						process_mod(&items.1, w)?;
					}
				}
			}
			syn::Item::Struct(item) => {
				if item.fields.is_empty() {
					warn!("skipping empty struct: {}", item.ident);
					continue;
				}

				let reprc = parse_repr_c(&item.attrs)
					.map_err(|e| format!("failed to process struct {}: {e}", item.ident))?;

				assert!(
					reprc.base_type.is_none(),
					"structs must not have a repr type: {}",
					item.ident
				);

				if let Some(doc) = extract_doc_comment(&item.attrs, 0) {
					writeln!(w, "{doc}")?;
				} else {
					warn!("missing documentation for struct: {}", item.ident);
				}

				let ident = to_hungarian(&item.ident.to_string());

				if let Some(align) = reprc.alignment {
					write!(w, "ORO_BOOT_ALIGN({align}, ")?;
				}
				writeln!(w, "typedef struct {ident} {{")?;
				for field in &item.fields {
					let Some(field_ident) = field.ident.as_ref() else {
						panic!("struct fields must have an identifier: {}", item.ident);
					};

					if !matches!(field.vis, syn::Visibility::Public(_)) {
						return Err(format!(
							"struct fields must be public: {}::{}",
							item.ident, field_ident
						)
						.into());
					}

					let ty = type_to_ctype(&field.ty).map_err(|e| {
						format!(
							"failed to process struct field {}::{}: {e}",
							item.ident,
							field.ident.as_ref().unwrap()
						)
					})?;

					if let Some(doc) = extract_doc_comment(&field.attrs, 1) {
						writeln!(w, "{doc}")?;
					} else {
						warn!(
							"missing documentation for struct field: {}::{}",
							item.ident,
							field.ident.as_ref().unwrap()
						);
					}

					let (base, arr) = ty;
					writeln!(w, "\t{base} {}{arr};", field.ident.as_ref().unwrap())?;
				}
				write!(w, "}}")?;
				if reprc.alignment.is_some() {
					write!(w, ")")?;
				}
				writeln!(w, " {ident};\n\n")?;
			}
			syn::Item::Enum(item) => {
				if item.variants.is_empty() {
					warn!("skipping empty enum: {}", item.ident);
					continue;
				}

				let reprc = parse_repr_c(&item.attrs)
					.map_err(|e| format!("failed to process enum {}: {e}", item.ident))?;

				let Some(repr_type) = &reprc.base_type else {
					return Err(format!("enums must have a repr type: {}", item.ident).into());
				};

				if reprc.alignment.is_some() {
					return Err(format!(
						"enums cannot have an alignment (and must be marked #[repr(C)]): {}",
						item.ident
					)
					.into());
				}

				// SAFETY(qix-): repr_type is guaranteed to be a valid type since we check it in parse_repr_c
				let (ctype, arr) = type_to_ctype(repr_type).unwrap();
				assert_eq!(arr, "");

				if let Some(doc) = extract_doc_comment(&item.attrs, 0) {
					writeln!(w, "{doc}")?;
				} else {
					warn!("missing documentation for enum: {}", item.ident);
				}

				let ident = to_hungarian(&item.ident.to_string());
				let enum_ident = item.ident.to_string().to_case(Case::ScreamingSnake);

				writeln!(w, "#ifdef __cplusplus")?;
				writeln!(w, "enum {ident} : {ctype} {{")?;
				writeln!(w, "#else")?;
				writeln!(w, "typedef {ctype} {ident};")?;
				writeln!(w, "enum {ident} {{")?;
				writeln!(w, "#endif")?;
				for variant in &item.variants {
					assert!(
						variant.fields.is_empty(),
						"enums with fields are not supported: {}::{}",
						item.ident,
						variant.ident
					);

					let Some((_, disc)) = &variant.discriminant else {
						panic!(
							"enums without discriminants are not supported: {}::{}",
							item.ident, variant.ident
						);
					};

					let syn::Expr::Lit(syn::ExprLit { lit, .. }) = disc else {
						panic!(
							"enum discriminants must be literals: {}::{}",
							item.ident, variant.ident
						);
					};

					let syn::Lit::Int(disc) = lit else {
						panic!(
							"enum discriminants must be integers: {}::{}",
							item.ident, variant.ident
						);
					};

					if let Some(doc) = extract_doc_comment(&variant.attrs, 1) {
						writeln!(w, "{doc}")?;
					} else {
						warn!(
							"missing documentation for enum variant: {}::{}",
							item.ident, variant.ident
						);
					}

					let ident = variant.ident.to_string().to_case(Case::ScreamingSnake);

					writeln!(
						w,
						"\tORO_BOOT_ENUM({enum_ident}, {ident}) = {},",
						disc.base10_digits()
					)?;
				}
				writeln!(w, "}};\n\n")?;
			}
			syn::Item::Union(item) => {
				if item.fields.named.is_empty() {
					warn!("skipping empty union: {}", item.ident);
					continue;
				}

				let reprc = parse_repr_c(&item.attrs)
					.map_err(|e| format!("failed to process union {}: {e}", item.ident))?;

				assert!(
					reprc.base_type.is_none(),
					"unions must not have a repr type: {}",
					item.ident
				);

				if let Some(doc) = extract_doc_comment(&item.attrs, 0) {
					writeln!(w, "{doc}")?;
				} else {
					warn!("missing documentation for union: {}", item.ident);
				}

				let ident = to_hungarian(&item.ident.to_string());

				if let Some(align) = reprc.alignment {
					write!(w, "ORO_BOOT_ALIGN({align}, ")?;
				}
				writeln!(w, "typedef union {ident} {{")?;
				for field in &item.fields.named {
					let Some(field_ident) = field.ident.as_ref() else {
						panic!("union fields must have an identifier: {}", item.ident);
					};

					if !matches!(field.vis, syn::Visibility::Public(_)) {
						return Err(format!(
							"union fields must be public: {}::{}",
							item.ident, field_ident
						)
						.into());
					}

					let ty = type_to_ctype(&field.ty).map_err(|e| {
						format!(
							"failed to process union field {}::{}: {e}",
							item.ident,
							field.ident.as_ref().unwrap()
						)
					})?;

					if let Some(doc) = extract_doc_comment(&field.attrs, 1) {
						writeln!(w, "{doc}")?;
					} else {
						warn!(
							"missing documentation for union field: {}::{}",
							item.ident,
							field.ident.as_ref().unwrap()
						);
					}

					let (base, arr) = ty;
					writeln!(w, "\t{base} {}{arr};", field.ident.as_ref().unwrap())?;
				}
				write!(w, "}}")?;
				if reprc.alignment.is_some() {
					write!(w, ")")?;
				}
				writeln!(w, " {ident};\n\n")?;
			}
			_ => {}
		}
	}
	Ok(())
}

#[derive(Default)]
struct ReprC {
	base_type: Option<syn::Type>,
	// NOTE(qix-): We could, but I excluded it for now. See above.
	// packed: bool,
	alignment: Option<usize>,
}

fn parse_repr_c(attrs: &[syn::Attribute]) -> Result<ReprC> {
	for attr in attrs {
		if attr.path().is_ident("repr") {
			let mut found_repr = false;
			let mut reprc = ReprC::default();

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("C") {
					found_repr = true;
					return Ok(());
				}

				if let Some(ident) = meta.path.get_ident() {
					let ident = ident.to_string();
					if VALID_REPR_TYPES.contains(&ident.as_str()) {
						reprc.base_type = Some(syn::parse_str(&ident).unwrap());
						found_repr = true;
						return Ok(());
					}
				}

				// NOTE(qix-): We could, but I excluded it for now. See above.
				// reprc.packed = true;
				// return Ok(());
				assert!(
					!meta.path.is_ident("packed"),
					"packed types are not supported"
				);

				if meta.path.is_ident("align") {
					let content;
					syn::parenthesized!(content in meta.input);
					let lit: syn::LitInt = content.parse()?;
					let n: usize = lit.base10_parse()?;
					reprc.alignment = Some(n);
					return Ok(());
				}

				warn!(
					"unknown repr attribute: {}",
					meta.path.get_ident().unwrap().to_string()
				);

				Ok(())
			})?;

			if !found_repr {
				return Err(
					"found #[repr] attribute but was not marked 'repr(C)' or 'repr($ty)'".into(),
				);
			}

			return Ok(reprc);
		}
	}

	Err("missing #[repr(C)] attribute".into())
}

fn type_to_ctype(ty: &syn::Type) -> Result<(String, String)> {
	match ty {
		syn::Type::Path(syn::TypePath { path, .. }) => {
			if path.segments.len() == 1 {
				let seg = path.segments.last().unwrap();

				let prim = match seg.ident.to_string().as_str() {
					"u8" => Some("uint8_t"),
					"u16" => Some("uint16_t"),
					"u32" => Some("uint32_t"),
					"u64" => Some("uint64_t"),
					"i8" => Some("int8_t"),
					"i16" => Some("int16_t"),
					"i32" => Some("int32_t"),
					"i64" => Some("int64_t"),
					sz @ ("isize" | "usize" | "f32" | "f64" | "bool") => {
						return Err(format!("{sz} is not supported").into());
					}
					_ => None,
				};

				if let Some(prim) = prim {
					return Ok((prim.into(), String::new()));
				}
			}

			let seg = path.segments.last().unwrap();

			if seg.ident == "ManuallyDrop" {
				let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
					args,
					..
				}) = &seg.arguments
				else {
					unreachable!();
				};
				let syn::GenericArgument::Type(ty) = args.first().unwrap() else {
					unreachable!();
				};
				return type_to_ctype(ty);
			}

			Ok((to_hungarian(&seg.ident.to_string()), String::new()))
		}
		syn::Type::Array(syn::TypeArray { elem, len, .. }) => {
			let syn::Expr::Lit(syn::ExprLit {
				lit: syn::Lit::Int(len),
				..
			}) = len
			else {
				return Err("array length must be a literal integer".into());
			};

			let (base, arr) = type_to_ctype(elem)?;

			if !arr.is_empty() {
				return Err("nested arrays are not supported".into());
			}

			Ok((base, format!("[{len}]")))
		}
		_ => Err(format!("unsupported type: {ty:?}").into()),
	}
}

fn to_hungarian(s: &str) -> String {
	let mut ident = "oro_".to_owned() + &s.to_case(Case::Snake) + "_t";

	// Super specific special handling: any case of "v_n" where n is a digit
	// should be replaced with "vn".
	let idx = ident.find("_v_").unwrap_or(usize::MAX);
	if idx != usize::MAX {
		let mut chars = ident.chars().collect::<Vec<_>>();
		chars.remove(idx + 2);
		ident = chars.into_iter().collect();
	}

	ident
}

fn extract_doc_comment(attrs: &[syn::Attribute], indent_level: usize) -> Option<String> {
	let mut lines = Vec::new();
	for attr in attrs {
		if attr.path().is_ident("doc") {
			if let syn::Meta::NameValue(syn::MetaNameValue {
				value: syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Str(lit),
					..
				}),
				..
			}) = &attr.meta
			{
				// We do this so that `.lines()` doesn't drop leading/trailing empty lines,
				// giving us the exact output as the original source.
				let doc = " ".to_owned() + &lit.value() + " ";
				let new_lines = doc
					.lines()
					.map(|s| {
						// Special handling: remove any instances of "super::".
						s.replace("super::", "")
					})
					.map(|s| s.trim().to_owned())
					.collect::<Vec<_>>();
				lines.extend(new_lines);
			}
		}
	}

	if lines.is_empty() {
		None
	} else if lines.len() == 1 {
		Some(format!("{}/** {} */", "\t".repeat(indent_level), lines[0]))
	} else {
		for line in &mut lines {
			line.insert_str(0, &"\t".repeat(indent_level + 1));
		}
		lines.insert(0, format!("{}/**", "\t".repeat(indent_level)));
		lines.push(format!("{}*/", "\t".repeat(indent_level)));
		Some(lines.join("\n"))
	}
}
