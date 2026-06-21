use std::{collections::HashSet, io::Write};

use clap::Parser;

use crate::vfs::ConfigToml;

#[derive(Parser)]
pub struct Args {
	/// Check that vendoring would not change the build output for these artifacts.
	#[arg(short = 'c', long = "check")]
	check: bool,
}

pub fn run(args: Args) {
	if args.check {
		check();
	} else {
		vendor();
	}
}

fn vendor() {
	let mut cmd = super::cargo();
	cmd.arg("vendor").arg("--color").arg("always");

	let status = cmd.status().expect("failed to execute cargo vendor");

	std::io::stdout().flush().ok();
	std::io::stderr().flush().ok();

	if !status.success() {
		eprintln!("cargo vendor failed");
		std::process::exit(1);
	}

	let vfs = crate::vfs::Vfs::new_from_cargo();
	let mut config_toml = vfs.config_toml();
	config_toml.set_vendor_paths(vfs.lockfile());

	vfs.write_config_toml(config_toml);

	eprintln!();
	eprintln!("wrote .cargo/config.toml paths");

	super::patch::apply_all();

	eprintln!("cleaning vendor directory...");
	match std::process::Command::new("git")
		.arg("clean")
		.arg("-dffX")
		.arg("vendor")
		.current_dir(vfs.root())
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.stdin(std::process::Stdio::inherit())
		.status()
	{
		Ok(status) if status.success() => {
			eprintln!("cleaned vendor/ directory");
		}
		Ok(status) => {
			panic!("git exited non-zero; is it installed?: {status}");
		}
		Err(err) => {
			panic!("failed to spawn git to clean vendor/directory: {err}");
		}
	}
}

fn check() {
	let vfs = crate::vfs::Vfs::new_from_cargo();

	let mut ok = true;

	let mut expected_toplevel_deps = vfs
		.root_cargo_toml()
		.dependencies
		.unwrap_or_default()
		.into_iter()
		.filter_map(|(k, d)| {
			d.version().and_then(|version| {
				if let Some(version) = version.strip_prefix('=') {
					let version = version.trim();
					if version.is_empty() {
						eprintln!("error: top-level dependency {k} is malformed");
						ok = false;
						None
					} else {
						Some((k, version.to_string()))
					}
				} else {
					eprintln!(
						"error: top-level dependency {k} does not have an exact version \
						 requirement"
					);
					ok = false;
					None
				}
			})
		})
		.collect::<HashSet<_>>();

	for vendor in vfs.vendor_packages() {
		let version = if let Some((version, _)) = vendor.manifest.package.version.split_once('+') {
			version
		} else {
			&vendor.manifest.package.version
		};

		expected_toplevel_deps.remove(&(vendor.manifest.package.name, version.to_string()));
	}

	for (name, version) in expected_toplevel_deps {
		eprintln!("error: top-level dependency would change vendors: {name} {version}");
		ok = false;
	}

	let mut config_toml = vfs.config_toml();
	let original_config_toml = config_toml.clone();
	config_toml.set_vendor_paths(vfs.lockfile());
	if original_config_toml != config_toml {
		eprintln!(
			"error: `cargo oro vendor` would change .cargo/config.toml but --check was passed"
		);
		ok = false;
	}

	if !ok {
		std::process::exit(1);
	}

	eprintln!("vendors OK; checking patches");

	super::patch::check_all();
}
