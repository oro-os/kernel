use std::io::Write;

use clap::Parser;

use crate::vfs::ConfigToml;

#[derive(Parser)]
pub struct Args {
	/// Check that vendoring would not change the build output for these artifacts.
	#[arg(short = 'c', long = "check")]
	check: bool,
}

pub fn run(args: Args) {
	let mut cmd = super::cargo();
	cmd.arg("vendor").arg("--color").arg("always");

	if args.check {
		cmd.arg("--locked").arg("--offline").arg("--quiet");
	}

	let status = cmd.status().expect("failed to execute cargo vendor");

	std::io::stdout().flush().ok();
	std::io::stderr().flush().ok();

	if !status.success() {
		eprintln!("cargo vendor failed");
		std::process::exit(1);
	}

	let vfs = crate::vfs::Vfs::new_from_cargo();
	let mut config_toml = vfs.config_toml();
	if args.check {
		let original_config_toml = config_toml.clone();
		config_toml.set_vendor_paths(vfs.lockfile());
		if original_config_toml != config_toml {
			panic!("`cargo oro vendor` would change .cargo/config.toml but --check was passed");
		}

		eprintln!(".cargo/config.toml paths match");
	} else {
		config_toml.set_vendor_paths(vfs.lockfile());
		vfs.write_config_toml(config_toml);
		eprintln!("wrote .cargo/config.toml paths");
	}
}
