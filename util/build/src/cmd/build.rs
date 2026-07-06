use std::io::Write;

use clap::Parser;

use crate::artifact::ArtifactExt;

#[derive(Parser)]
pub struct Args {
	/// Which artifacts to build.
	#[clap(value_parser = crate::artifact::Parser::new())]
	artifacts: Vec<Vec<crate::vfs::Artifact>>,

	/// Whether to build in release mode.
	#[clap(long)]
	release: bool,
}

pub fn run(args: Args) {
	let artifacts = args.artifacts.collapse();

	let mut ok = true;

	for artifact in artifacts {
		let mut cmd = super::cargo();
		cmd.arg("--color")
			.arg("always")
			.arg("build")
			// TODO: reenable after fix: https://github.com/rust-lang/cargo/issues/7058
			//.arg("--frozen")
			.current_dir(&artifact.path);

		if args.release {
			cmd.arg("--release");
		}

		let status = cmd.status().expect(&format!(
			"failed to execute cargo build for artifact {}: {}",
			artifact.name,
			artifact.path.display()
		));

		std::io::stdout().flush().ok();
		std::io::stderr().flush().ok();

		if !status.success() {
			eprintln!(
				"cargo build failed for artifact {}: {}",
				artifact.name,
				artifact.path.display()
			);

			ok = false;
		}
	}

	if !ok {
		std::process::exit(1);
	}
}
