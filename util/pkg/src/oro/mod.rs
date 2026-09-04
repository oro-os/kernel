//! The `oro` Koto module, importable from every packaging script.

pub mod artifact;
pub mod file;
pub mod iso;
pub mod limine;
pub mod package;
pub mod ramdisk;

use std::path::Path;

use koto::prelude::*;

/// Creates the `oro` module for the given workspace.
pub fn make_module(workspace_root: &Path) -> KMap {
	let map = KMap::with_type("oro");
	map.insert("package", package::make_module());
	map.insert("artifact", artifact::make_module(workspace_root));
	map.insert("file", file::make_module());
	map.insert("iso", iso::make_module());
	map.insert("limine", limine::make_module());
	map.insert("ramdisk", ramdisk::make_module());
	map
}
