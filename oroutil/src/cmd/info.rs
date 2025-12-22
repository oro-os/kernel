//! Implements info command for displaying workspace crate information.

use crate::crate_info::{ArtifactType, WorkspaceCrates, WorkspaceMap};

pub fn run(_args: crate::InfoArgs) -> Result<(), Box<dyn std::error::Error>> {
	let workspace = WorkspaceCrates::load()?;
	let map = WorkspaceMap::from_crates(&workspace);

	// Workspace targets
	println!("Configured Targets:");
	if workspace.workspace_metadata.targets.is_empty() {
		println!("  (none)");
	} else {
		for (name, target) in &workspace.workspace_metadata.targets {
			println!("  {}", name);
			println!("    Target JSON: {}", target.target_json);
			if !target.features.is_empty() {
				println!("    Features: {}", target.features.join(", "));
			}
		}
	}
	println!();

	// Host crates
	println!("Host Crates ({}):", map.host.len());
	if map.host.is_empty() {
		println!("  (none)");
	} else {
		for name in &map.host {
			let info = &workspace.crates[name];

			let no_std_str = if info.oro_metadata.no_std.unwrap_or(false) {
				" [no_std]"
			} else {
				""
			};

			println!("  {} ({}{})", name, info.type_str(), no_std_str);
		}
	}
	println!();

	// Architecture-specific crates
	for arch in workspace.workspace_metadata.targets.keys() {
		if let Some(crates) = map.by_arch.get(arch) {
			println!("Crates for {} ({}):", arch, crates.len());
			for name in crates {
				let info = &workspace.crates[name];

				let no_std_str = if info.oro_metadata.no_std.unwrap_or(false) {
					" [no_std]"
				} else {
					""
				};

				let artifact_str = match info.oro_metadata.artifact {
					Some(ArtifactType::Kernel) => " [kernel]",
					Some(ArtifactType::Bootloader) => " [bootloader]",
					None => "",
				};

				println!(
					"  {} ({}{}{})",
					name,
					info.type_str(),
					no_std_str,
					artifact_str
				);
			}
			println!();
		}
	}

	// Kernels
	if !map.kernels.is_empty() {
		println!("Kernel Artifacts:");
		for (arch, crates) in &map.kernels {
			for name in crates {
				let info = &workspace.crates[name];
				if let Some(bins) = &info.oro_metadata.bins {
					if let Some(bin_name) = bins.get(arch) {
						println!("  {} -> {}", name, bin_name);
					}
				}
			}
		}
		println!();
	}

	// Bootloaders
	if !map.bootloaders.is_empty() {
		println!("Bootloader Artifacts:");
		for (arch, crates) in &map.bootloaders {
			for name in crates {
				let info = &workspace.crates[name];
				if let Some(bins) = &info.oro_metadata.bins {
					if let Some(bin_name) = bins.get(arch) {
						println!("  {} -> {}", name, bin_name);
					}
				}
			}
		}
		println!();
	}

	// Auto-lib crates (no explicit target, library only)
	let auto_lib_crates = workspace.auto_lib_crates();

	if !auto_lib_crates.is_empty() {
		println!("Auto-Lib Crates ({}):", auto_lib_crates.len());
		for info in &auto_lib_crates {
			let no_std_str = if info.oro_metadata.no_std.unwrap_or(false) {
				" [no_std]"
			} else {
				""
			};

			println!(
				"  {} ({}{})",
				info.package.name,
				info.type_str(),
				no_std_str
			);
		}
	}

	Ok(())
}
