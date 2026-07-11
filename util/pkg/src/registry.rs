//! Discovery and loading of the packaging scripts under `pkg/`.
//!
//! Every `.koto` file under `pkg/platform` is loaded in its own Koto
//! context; each of its exports is a buildable package artifact named
//! after the export's key.
//!
//! Before a script is run, its context is prepared with:
//!
//! - the `oro` module in the prelude;
//! - `pkg/contrib/_prelude.koto`, run directly in the context;
//! - the remaining `pkg/contrib/*.koto` files, each run as a module and
//!   made importable via `oro_contrib.<file stem>`.

use std::path::{Path, PathBuf};

use koto::{CompileArgs, Koto, KotoSettings, prelude::*};

/// A loaded platform script and the package exports it declared.
pub struct LoadedScript {
	/// The script's path, relative to the workspace root where possible.
	pub path: PathBuf,
	/// The script's Koto context, retained so that its exported
	/// functions can be called at build time.
	pub koto: Koto,
	/// The script's exports: package names mapped to their constructor
	/// functions.
	pub exports: Vec<(String, KValue)>,
}

/// The set of all discovered packages.
pub struct Registry {
	pub scripts: Vec<LoadedScript>,
}

impl Registry {
	/// Crawls `pkg/platform` and loads every packaging script.
	pub fn load(workspace_root: &Path) -> Result<Self, String> {
		let platform_dir = workspace_root.join("pkg").join("platform");

		let mut script_paths = Vec::new();
		crawl_scripts(&platform_dir, &mut script_paths)?;

		let mut scripts = Vec::new();
		for path in script_paths {
			scripts.push(load_script(workspace_root, &path)?);
		}

		// Package names are the artifact namespace; require them to be
		// unique across all scripts.
		let mut seen = std::collections::HashMap::<&str, &Path>::new();
		for script in &scripts {
			for (name, _) in &script.exports {
				if let Some(previous) = seen.insert(name, &script.path) {
					return Err(format!(
						"duplicate package '{name}' exported by both {} and {}",
						previous.display(),
						script.path.display()
					));
				}
			}
		}

		Ok(Self { scripts })
	}
}

/// Recursively collects `.koto` files, skipping `_`-prefixed entries.
///
/// Files are visited in sorted order so that discovery is deterministic.
fn crawl_scripts(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
	let mut entries: Vec<_> = std::fs::read_dir(dir)
		.map_err(|e| format!("failed to read {}: {e}", dir.display()))?
		.filter_map(|entry| entry.ok())
		.map(|entry| entry.path())
		.filter(|path| {
			path.file_name()
				.is_some_and(|name| !name.to_string_lossy().starts_with('_'))
		})
		.collect();
	entries.sort();

	for path in entries {
		if path.is_dir() {
			crawl_scripts(&path, out)?;
		} else if path.extension().is_some_and(|ext| ext == "koto") {
			out.push(path);
		}
	}

	Ok(())
}

/// Loads a single platform script into a fresh Koto context.
fn load_script(workspace_root: &Path, script_path: &Path) -> Result<LoadedScript, String> {
	let mut koto = Koto::with_settings(KotoSettings {
		run_tests: false,
		..Default::default()
	});

	koto.prelude()
		.insert("oro", crate::oro::make_module(workspace_root));

	let contrib_dir = workspace_root.join("pkg").join("contrib");

	// The contrib prelude runs directly in the script's context, so that
	// it can extend the core library (e.g. `iterator.join`).
	let prelude_path = contrib_dir.join("_prelude.koto");
	if prelude_path.is_file() {
		run_file(&mut koto, &prelude_path)?;
	}

	// The remaining contrib files are exposed as `oro_contrib.<stem>`.
	//
	// TODO(pkg): contrib modules are loaded in sorted filename order and
	// cannot yet import one another.
	let contrib = KMap::with_type("oro_contrib");
	let mut contrib_paths = Vec::new();
	if contrib_dir.is_dir() {
		crawl_scripts(&contrib_dir, &mut contrib_paths)?;
	}
	for path in contrib_paths {
		let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
			continue;
		};
		koto.exports_mut().clear();
		run_file(&mut koto, &path)?;
		contrib.insert(stem.as_str(), snapshot_exports(&koto));
	}
	koto.prelude().insert("oro_contrib", contrib);

	koto.exports_mut().clear();
	run_file(&mut koto, script_path)?;

	let mut exports = Vec::new();
	for (key, value) in koto.exports().data().iter() {
		let KValue::Str(name) = key.value() else {
			continue;
		};
		if !value.is_callable() {
			return Err(format!(
				"{}: export '{name}' is not a function; package exports must be functions \
				 returning a package definition",
				script_path.display()
			));
		}
		exports.push((name.to_string(), value.clone()));
	}

	Ok(LoadedScript {
		path: script_path
			.strip_prefix(workspace_root)
			.map(Path::to_path_buf)
			.unwrap_or_else(|_| script_path.to_path_buf()),
		koto,
		exports,
	})
}

/// Copies the context's current exports into a standalone map.
///
/// A plain clone would share the underlying data with the context's
/// exports map, which is cleared between module loads.
fn snapshot_exports(koto: &Koto) -> KMap {
	let map = KMap::new();
	for (key, value) in koto.exports().data().iter() {
		map.insert(key.clone(), value.clone());
	}
	map
}

fn run_file(koto: &mut Koto, path: &Path) -> Result<KValue, String> {
	let script = std::fs::read_to_string(path)
		.map_err(|e| format!("failed to read {}: {e}", path.display()))?;

	koto.compile_and_run(CompileArgs {
		script: &script,
		script_path: Some(path.to_string_lossy().to_string().into()),
		compiler_settings: Default::default(),
	})
	.map_err(|e| format!("{}: {e}", path.display()))
}
