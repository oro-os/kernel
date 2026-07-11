//! Executes package definitions, producing artifacts under `target/pkg`.
//!
//! The executor is deliberately thin: it resolves a package export to
//! its definition, computes the output path, and hands that path to the
//! script's build function. Everything else — assembling images,
//! post-processing them, and so on — is the script's responsibility,
//! via the actions exposed in the `oro` module.

use std::path::{Path, PathBuf};

use koto::{Koto, prelude::*};

use crate::oro::package::{Package, PackageOutput};

/// Builds the named package export, returning the path of the produced
/// artifact.
pub fn build(
	koto: &mut Koto,
	workspace_root: &Path,
	name: &str,
	constructor: &KValue,
) -> Result<PathBuf, String> {
	// Evaluate the export to get the package definition.
	let package = koto
		.call_function(constructor.clone(), &[])
		.map_err(|e| format!("package '{name}': {e}"))?;
	let KValue::Object(package) = package else {
		return Err(format!(
			"package '{name}': export did not return a package definition"
		));
	};
	let package = package
		.cast::<Package>()
		.map_err(|e| format!("package '{name}': {e}"))?
		.clone();

	let Some(output) = package.output else {
		return Err(format!(
			"package '{name}': package declares no output (e.g. via `single_file`)"
		));
	};

	match output {
		PackageOutput::SingleFile { extension, thunk } => {
			let out_dir = workspace_root.join("target").join("pkg");
			std::fs::create_dir_all(&out_dir)
				.map_err(|e| format!("failed to create {}: {e}", out_dir.display()))?;
			let out_path = out_dir.join(format!("{name}.{extension}"));

			let out_arg = KValue::Str(out_path.to_string_lossy().to_string().into());
			koto.call_function(thunk, CallArgs::Single(out_arg))
				.map_err(|e| format!("package '{name}': {e}"))?;

			if !out_path.is_file() {
				return Err(format!(
					"package '{name}': build function did not produce {}",
					out_path.display()
				));
			}

			Ok(out_path)
		}
	}
}
