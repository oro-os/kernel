//! Provides a macro for producing a buffer of assembly instructions
//! using the `asm!` macro.
#![allow(clippy::items_after_statements)]

use quote::quote;
use std::io::Read;

#[allow(clippy::missing_docs_in_private_items)]
pub fn asm_buffer(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let args = std::env::args().collect::<Vec<_>>();

	let target_args = args
		.as_slice()
		.chunks(2)
		.filter(|chunk| {
			chunk.len() == 2
				&& chunk.first().map_or(false, |t| {
					t == "--target" || t == "--extern" || t == "-L" || t == "--check-cfg"
				})
		})
		.flatten()
		.collect::<Vec<_>>();

	// Generate a random (enough) nonce
	let nonce = tokens.clone().into_iter().fold(1usize, |acc, t| {
		acc.wrapping_mul(t.span().line())
			.wrapping_add(t.span().column())
	});

	let mut tokens: Vec<proc_macro::TokenTree> = tokens.into_iter().collect();
	if tokens.last().map_or(false, |t| t.to_string() == ",") {
		tokens.pop();
	}
	let tokens: proc_macro2::TokenStream = tokens
		.into_iter()
		.collect::<proc_macro::TokenStream>()
		.into();

	let src: String = quote::quote! {
		#![no_std]
		#![no_main]
		#![feature(naked_functions)]
		#[link_section = ".inline_asm"]
		#[no_mangle]
		#[naked]
		#[allow(non_snake_case)]
		pub extern "C" fn _INLINE_ASM() {
			unsafe {
				::core::arch::asm!(
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					#tokens,
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					options(noreturn)
				);
			}
		}
	}
	.to_string();

	let tmp_path = std::env::temp_dir().join("oro_asm_buffer");
	std::fs::create_dir(&tmp_path)
		.or_else(|e| {
			match e.kind() {
				std::io::ErrorKind::AlreadyExists => Ok(()),
				_ => Err(e),
			}
		})
		.expect("failed to create temporary directory");

	// Dump it to a file.
	let input_path = tmp_path.join(format!("asm-{nonce:X}.rs"));
	std::fs::write(&input_path, src).expect("failed to write source file");

	let output_path = tmp_path.join(format!("asm-{nonce:X}.so"));

	// Compile it with rustc.
	let rustc = std::env::var("RUSTC").unwrap_or("rustc".to_string());

	let mut cmd = std::process::Command::new(rustc);
	cmd.current_dir(&tmp_path).args([
		"-C",
		"link-arg=-static",
		"-C",
		"link-arg=-mrelax",
		"-C",
		"link-arg=-Bstatic",
		"--crate-type=rlib",
		"-Z",
		"unstable-options",
		"-C",
		"panic=abort",
		"--edition=2021",
		"-o",
		output_path.to_string_lossy().as_ref(),
		input_path.to_string_lossy().as_ref(),
	]);

	cmd.args(target_args);

	cmd.status().expect("failed to compile assembly buffer");

	// Open up the file and read it into a buffer.
	let mut file =
		std::fs::File::open(&output_path).expect("failed to open compiled assembly file");
	let mut buffer = Vec::new();
	file.read_to_end(&mut buffer)
		.expect("failed to read back compiled assembly");

	// Search through the buffer until we find three `0xDEADBEEF`'s.
	// They will always be in the order `0xDE`, `0xAD`, `0xBE`, `0xEF`,
	// regardless of system endianness.
	const DEADBEEF: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];
	let mut start = None;
	let mut end = None;
	let mut i = 0;
	'to_end: for (idx, reset_i) in [(&mut start, true), (&mut end, false)] {
		'outer: while i < buffer.len() {
			if buffer[i] == 0xDE {
				*idx = Some(i);

				for _ in 0..3 {
					for beef in &DEADBEEF {
						let byte = buffer[i];
						i += 1;
						if byte != *beef {
							*idx = None;
							continue 'outer;
						}
					}
				}

				// We found it.
				if reset_i {
					*idx = Some(i);
				}

				continue 'to_end;
			}

			i += 1;
		}
	}

	let start = start.expect("could not find start of assembly buffer");
	let end = end.expect("could not find end of assembly buffer");

	let bytes = &buffer[start..end];

	quote! {
		[
			#(#bytes),*
		]
	}
	.into()
}
