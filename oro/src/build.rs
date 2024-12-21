//! Houses the build configurator for Oro modules.
//!
//! # Usage
//! In the module crate's `build.rs` script, call the `build` function to configure the linker to
//! generate a valid Oro module that can be loaded by the Oro kernel.
//!
//! ```no_run
//! fn main() {
//! 	::oro::build();
//! }
//! ```

use std::path::PathBuf;

/// To be called by the module crate's `build.rs` script.
///
/// This function configures the linker to generate a valid
/// Oro module that can be loaded by the Oro kernel.
pub fn build() {
	let target = std::env::var("TARGET").unwrap();

	match target.as_str() {
		"aarch64-unknown-none" => {
			let linker_script = PathBuf::from(file!())
				.parent()
				.unwrap()
				.parent()
				.unwrap()
				.join("aarch64.ld");

			println!("cargo:rustc-link-arg=-static");
			println!("cargo:rustc-link-arg=--relax");
			println!("cargo:rustc-link-arg=-T");
			println!("cargo:rustc-link-arg={}", linker_script.display());
		}
		"x86_64-unknown-none" => {
			let linker_script = PathBuf::from(file!())
				.parent()
				.unwrap()
				.parent()
				.unwrap()
				.join("x86_64.ld");

			println!("cargo:rustc-link-arg=-static");
			println!("cargo:rustc-link-arg=--relax");
			println!("cargo:rustc-link-arg=-T");
			println!("cargo:rustc-link-arg={}", linker_script.display());
		}
		_ => {
			panic!(
				"unsupported target when building Oro module: {target}; run with \
				 `--target=<arch>-unknown-none` instead"
			);
		}
	}
}
