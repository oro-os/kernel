//! Crate metadata and classification for build orchestration.

use std::collections::HashMap;

use cargo_metadata::{Metadata, Package, TargetKind};
use serde::Deserialize;

/// Target specification for a crate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Target {
	/// Single target (host or architecture name).
	Single(String),
	/// Multiple targets.
	Multiple(Vec<String>),
}

impl Target {
	/// Returns true if this target includes the host.
	pub fn includes_host(&self) -> bool {
		match self {
			Target::Single(s) => s == "host",
			Target::Multiple(v) => v.iter().any(|s| s == "host"),
		}
	}

	/// Returns true if this target includes the given architecture.
	pub fn includes_arch(&self, arch: &str) -> bool {
		match self {
			Target::Single(s) => s == arch,
			Target::Multiple(v) => v.iter().any(|s| s == arch),
		}
	}

	/// Returns all architecture names (excluding "host").
	pub fn arch_names(&self) -> Vec<&str> {
		match self {
			Target::Single(s) if s != "host" => vec![s.as_str()],
			Target::Single(_) => vec![],
			Target::Multiple(v) => {
				v.iter()
					.filter(|s| s.as_str() != "host")
					.map(|s| s.as_str())
					.collect()
			}
		}
	}
}

/// Metadata specific to Oro kernel crates.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OroMetadata {
	/// Component type (e.g., "kernel", "limine").
	#[serde(default)]
	pub component: Option<String>,

	/// Target specification (host, arch name, or multiple).
	#[serde(default)]
	pub target: Option<Target>,

	/// Whether this crate is no_std.
	#[serde(default, rename = "no-std")]
	pub no_std: Option<bool>,

	/// Binary names for different architectures.
	#[serde(default)]
	pub bins: Option<HashMap<String, String>>,
}

/// Workspace target configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceTarget {
	/// Path to target JSON file (relative to workspace root).
	#[serde(rename = "target-json")]
	pub target_json: String,
	/// Features to enable for this target.
	#[serde(default)]
	pub features:    Vec<String>,
}

/// Workspace-level Oro metadata.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkspaceOroMetadata {
	/// Target configurations.
	#[serde(default)]
	pub target: HashMap<String, WorkspaceTarget>,
	/// Build-std crates to use.
	#[serde(default, rename = "build-std")]
	pub build_std: Option<Vec<String>>,
	/// Build-std features to enable.
	#[serde(default, rename = "build-std-features")]
	pub build_std_features: Option<Vec<String>>,
}

/// Information about a crate in the workspace.
#[derive(Debug, Clone)]
pub struct CrateInfo {
	/// The crate's package information.
	pub package:      Package,
	/// Oro-specific metadata.
	pub oro_metadata: OroMetadata,
	/// Whether this crate has binary targets.
	pub has_bins:     bool,
	/// Whether this crate has a library target.
	pub has_lib:      bool,
}

impl CrateInfo {
	/// Extracts Oro metadata from a package.
	fn from_package(package: Package) -> Self {
		let oro_metadata = package
			.metadata
			.get("oro")
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		let has_bins = package.targets.iter().any(|t| t.is_kind(TargetKind::Bin));
		let has_lib = package.targets.iter().any(|t| t.is_kind(TargetKind::Lib));

		CrateInfo {
			package,
			oro_metadata,
			has_bins,
			has_lib,
		}
	}

	/// Returns the effective target for this crate.
	/// If target is None (auto), infers based on whether it has bins.
	pub fn effective_target(&self) -> Option<Target> {
		match &self.oro_metadata.target {
			Some(t) => Some(t.clone()),
			None if self.has_bins => Some(Target::Single("host".to_string())),
			None => None, // Library follows compile target
		}
	}

	/// Returns true if this crate builds for host.
	pub fn builds_for_host(&self) -> bool {
		self.effective_target()
			.as_ref()
			.map_or(false, |t| t.includes_host())
	}

	/// Returns true if this crate builds for the given architecture.
	pub fn builds_for_arch(&self, arch: &str) -> bool {
		self.effective_target()
			.as_ref()
			.map_or(true, |t| t.includes_arch(arch)) // None = follows target
	}

	/// Returns a string describing the crate type (bin, lib, bin+lib).
	pub fn type_str(&self) -> &'static str {
		if self.has_bins && self.has_lib {
			"bin+lib"
		} else if self.has_bins {
			"bin"
		} else {
			"lib"
		}
	}
}

/// Collection of crates in the workspace with metadata.
#[derive(Debug)]
pub struct WorkspaceCrates {
	/// All crates in the workspace.
	pub crates: HashMap<String, CrateInfo>,
	/// Workspace-level Oro metadata.
	pub workspace_metadata: WorkspaceOroMetadata,
}

impl WorkspaceCrates {
	/// Loads workspace crate information from cargo metadata.
	pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
		let metadata: Metadata = crate::util::cargo_metadata()?;

		let workspace_metadata = metadata
			.workspace_metadata
			.get("oro")
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		let crates = metadata
			.workspace_packages()
			.into_iter()
			.map(|pkg| {
				let name = pkg.name.as_str().to_string();
				let info = CrateInfo::from_package(pkg.clone());
				(name, info)
			})
			.collect();

		Ok(WorkspaceCrates {
			crates,
			workspace_metadata,
		})
	}

	/// Returns crates that build for the given architecture (new metadata).
	pub fn for_arch(&self, arch: &str) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| c.builds_for_arch(arch))
			.collect()
	}

	/// Returns crates that build for host (new metadata).
	pub fn host_crates(&self) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| c.builds_for_host())
			.collect()
	}

	/// Returns library crates with no explicit target (follow compile target).
	pub fn auto_lib_crates(&self) -> Vec<&CrateInfo> {
		self.crates
			.values()
			.filter(|c| c.oro_metadata.target.is_none() && !c.has_bins)
			.collect()
	}
}

/// Categorized crates by build target.
#[derive(Debug, Default)]
pub struct WorkspaceMap {
	/// Crates that build for host.
	pub host:       Vec<String>,
	/// Crates that build for specific architectures.
	pub by_arch:    HashMap<String, Vec<String>>,
	/// Crates with components, organized by component -> arch -> crates.
	pub components: HashMap<String, HashMap<String, Vec<String>>>,
}

impl WorkspaceMap {
	/// Creates a workspace map from crate information.
	pub fn from_crates(workspace: &WorkspaceCrates) -> Self {
		let mut map = WorkspaceMap::default();

		for (name, info) in &workspace.crates {
			// Categorize by component type
			if let Some(component) = &info.oro_metadata.component {
				if let Some(target) = &info.oro_metadata.target {
					let arches = target.arch_names();
					for arch in arches {
						map.components
							.entry(component.clone())
							.or_default()
							.entry(arch.to_string())
							.or_default()
							.push(name.clone());
					}
				}
			}

			// Categorize by target
			if info.builds_for_host() {
				map.host.push(name.clone());
			}

			if let Some(target) = &info.oro_metadata.target {
				for arch in target.arch_names() {
					map.by_arch
						.entry(arch.to_string())
						.or_default()
						.push(name.clone());
				}
			}
		}

		map
	}
}
