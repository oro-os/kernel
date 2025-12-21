//! Crate metadata and classification for build orchestration.

use std::collections::HashMap;

use cargo_metadata::{Metadata, Package};
use serde::Deserialize;

/// Metadata specific to Oro kernel crates.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OroMetadata {
	/// Which architectures this crate requires.
	/// If empty or not specified, the crate is host-buildable.
	#[serde(default, rename = "requires-arch")]
	pub requires_arch: Vec<String>,

	/// Whether this crate requires `build-std`.
	/// If not specified, inferred from `requires_arch`.
	#[serde(default, rename = "requires-build-std")]
	pub requires_build_std: Option<bool>,
}

impl OroMetadata {
	/// Returns whether this crate requires build-std.
	pub fn needs_build_std(&self) -> bool {
		self.requires_build_std
			.unwrap_or(!self.requires_arch.is_empty())
	}

	/// Returns whether this crate can be built for the host architecture.
	pub fn is_host_buildable(&self) -> bool {
		self.requires_arch.is_empty()
	}

	/// Returns whether this crate can be built for the given architecture.
	pub fn supports_arch(&self, arch: crate::TargetArch) -> bool {
		if self.requires_arch.is_empty() {
			// Host-buildable crates can build for any arch
			true
		} else {
			self.requires_arch.iter().any(|a| a == &arch.to_string())
		}
	}
}

/// Information about a crate in the workspace.
#[derive(Debug, Clone)]
pub struct CrateInfo {
	/// The crate's package information.
	pub package:      Package,
	/// Oro-specific metadata.
	pub oro_metadata: OroMetadata,
}

impl CrateInfo {
	/// Extracts Oro metadata from a package.
	fn from_package(package: Package) -> Self {
		let oro_metadata = package
			.metadata
			.get("oro")
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		CrateInfo {
			package,
			oro_metadata,
		}
	}
}

/// Collection of crates in the workspace with metadata.
#[derive(Debug)]
pub struct WorkspaceCrates {
	/// All crates in the workspace.
	pub crates: HashMap<String, CrateInfo>,
}

impl WorkspaceCrates {
	/// Loads workspace crate information from cargo metadata.
	pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
		let metadata: Metadata = crate::util::cargo_metadata()?;

		let crates = metadata
			.workspace_packages()
			.into_iter()
			.map(|pkg| {
				let name = pkg.name.as_str().to_string();
				let info = CrateInfo::from_package(pkg.clone());
				(name, info)
			})
			.collect();

		Ok(WorkspaceCrates { crates })
	}

	/// Returns crates that can be built for the host.
	#[allow(dead_code)]
	pub fn host_buildable(&self) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| c.oro_metadata.is_host_buildable())
			.collect()
	}

	/// Returns crates that support the given architecture.
	/// Only returns crates that have oro metadata defined.
	pub fn for_arch(&self, arch: crate::TargetArch) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| {
				!c.oro_metadata.requires_arch.is_empty() && c.oro_metadata.supports_arch(arch)
			})
			.collect()
	}

	/// Returns crates that require a specific architecture.
	#[allow(dead_code)]
	pub fn arch_specific(&self) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| !c.oro_metadata.is_host_buildable())
			.collect()
	}
}
