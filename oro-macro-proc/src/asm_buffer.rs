//! Provides a macro for producing a buffer of assembly instructions
//! using the `asm!` macro.

use std::{env, io::Read, path::PathBuf};

use quote::quote;

#[expect(clippy::missing_docs_in_private_items)]
pub fn asm_buffer(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let args = std::env::args().collect::<Vec<_>>();

	let target_args = args
		.as_slice()
		.iter()
		.filter_pairs(|&[&first, _]| {
			first == "--target" || first == "--extern" || first == "-L" || first == "--check-cfg"
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
		pub extern "C" fn _inline_asm() {
			unsafe {
				::core::arch::naked_asm!(
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					#tokens,
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
					".byte 0xDE", ".byte 0xAD", ".byte 0xBE" , ".byte 0xEF",
				);
			}
		}
	}
	.to_string();

	let out_dir = PathBuf::from(env!("OUT_DIR"));
	let tmp_path = out_dir.join("oro_asm_buffer");
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

/// Implements the `filter_pairs()` method for an iterator.
trait IteratorFilterPairs: Sized + Iterator
where
	<Self as Iterator>::Item: Clone + Sized,
{
	/// Filters the iterator by a sliding window of two elements.
	fn filter_pairs<F>(self, predicate: F) -> FilterPairs<Self, F>
	where
		F: FnMut(&[&<Self as Iterator>::Item; 2]) -> bool;
}

impl<I> IteratorFilterPairs for I
where
	I: Iterator,
	I::Item: Clone + Sized,
{
	fn filter_pairs<F>(mut self, predicate: F) -> FilterPairs<Self, F>
	where
		F: FnMut(&[&I::Item; 2]) -> bool,
	{
		let next = self.next();
		FilterPairs {
			iter: self,
			predicate,
			buffer: [None, next],
		}
	}
}

/// An iterator that filters by a sliding window of two elements.
#[expect(clippy::missing_docs_in_private_items)]
struct FilterPairs<I, F>
where
	I: Iterator,
	F: FnMut(&[&I::Item; 2]) -> bool,
{
	iter:      I,
	predicate: F,
	buffer:    [Option<I::Item>; 2],
}

impl<I, F> Iterator for FilterPairs<I, F>
where
	I: Iterator,
	I::Item: Sized + Clone,
	F: FnMut(&[&I::Item; 2]) -> bool,
{
	type Item = [I::Item; 2];

	fn next(&mut self) -> Option<Self::Item> {
		self.buffer.swap(0, 1);
		self.buffer[1] = self.iter.next();

		while let [Some(a), Some(b)] = &self.buffer {
			if (self.predicate)(&[a, b]) {
				return Some([a.clone(), b.clone()]);
			}

			self.buffer.swap(0, 1);
			self.buffer[1] = self.iter.next();
		}

		None
	}
}
