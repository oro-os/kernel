//! The `oro.artifact` module; references to the workspace's Rust artifacts.

use std::path::Path;

use koto::prelude::*;
use orok_util_common::vfs::{Profile, Vfs};

use super::file::File;

/// Creates the `oro.artifact` module: a tree of the workspace's
/// discovered Rust artifacts, addressable as
/// `artifact.<kind>.<arch>.<dev|release>` (e.g. `artifact.kernel.x86_64.dev`).
///
/// Leaves are `File` references pointing at the artifact's binary under
/// `target/`; the binaries are only required to exist once a package
/// referencing them is built.
pub fn make_module(workspace_root: &Path) -> KMap {
	let map = KMap::with_type("artifact");

	for artifact in Vfs::new(workspace_root).artifacts() {
		let kind_map = match map.get(artifact.kind.as_str()) {
			Some(KValue::Map(existing)) => existing,
			_ => {
				let new = KMap::new();
				map.insert(artifact.kind.as_str(), new.clone());
				new
			}
		};

		let profiles = KMap::new();
		profiles.insert(
			"dev",
			File::disk(artifact.binary_path(workspace_root, Profile::Dev)),
		);
		profiles.insert(
			"release",
			File::disk(artifact.binary_path(workspace_root, Profile::Release)),
		);

		kind_map.insert(artifact.architecture.to_string().as_str(), profiles);
	}

	map
}
