//! Implements info command for displaying workspace crate information.

use crate::crate_info::{WorkspaceCrates, WorkspaceMap};

pub fn run(_args: crate::InfoArgs) -> Result<(), Box<dyn std::error::Error>> {
	let workspace = WorkspaceCrates::load()?;
	let map = WorkspaceMap::from_crates(&workspace);

	// Workspace targets
	println!("Configured Targets:");
	if workspace.workspace_metadata.target.is_empty() {
		println!("  (none)");
	} else {
		for (name, target) in &workspace.workspace_metadata.target {
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
	for arch in workspace.workspace_metadata.target.keys() {
		if let Some(crates) = map.by_arch.get(arch) {
			println!("Crates for {} ({}):", arch, crates.len());
			for name in crates {
				let info = &workspace.crates[name];

				let no_std_str = if info.oro_metadata.no_std.unwrap_or(false) {
					" [no_std]"
				} else {
					""
				};

				let component_str = if let Some(comp) = &info.oro_metadata.component {
					format!(" [{}]", comp)
				} else {
					String::new()
				};

				println!(
					"  {} ({}{}{})",
					name,
					info.type_str(),
					no_std_str,
					component_str
				);
			}
			println!();
		}
	}

	// Components with bins (dynamic)
	if !map.components.is_empty() {
		let mut sorted_components: Vec<_> = map.components.keys().collect();
		sorted_components.sort();

		for component in sorted_components {
			if let Some(arch_map) = map.components.get(component) {
				println!("Component '{}' binaries:", component);
				for (arch, crates) in arch_map {
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
		}
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
