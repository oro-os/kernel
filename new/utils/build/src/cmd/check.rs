use std::io::Write;

use clap::Parser;

use crate::artifact::ArtifactExt;

#[derive(Parser)]
pub struct Args {
	/// Which artifacts to check.
	#[clap(value_parser = crate::artifact::Parser::new())]
	artifacts: Vec<Vec<crate::vfs::Artifact>>,

	/// The profile to use.
	#[clap(long)]
	profile: Option<String>,

	/// Outputs JSON instead of human-readable text.
	#[clap(long)]
	json: bool,
}

pub fn run(args: Args) {
	let artifacts = args.artifacts.collapse();

	for artifact in artifacts {
		let mut cmd = super::cargo();
		cmd.arg("check").arg("--frozen").current_dir(&artifact.path);

		if let Some(profile) = &args.profile {
			cmd.arg("--profile").arg(profile);
		}

		if args.json {
			cmd.arg("--message-format")
				.arg("json")
				.arg("--color")
				.arg("never")
				.arg("--quiet");
		} else {
			cmd.arg("--color").arg("always");
		}

		let status = cmd.status().expect(&format!(
			"failed to execute cargo clippy for artifact {}: {}",
			artifact.name,
			artifact.path.display()
		));

		std::io::stdout().flush().ok();
		std::io::stderr().flush().ok();

		if !status.success() {
			eprintln!(
				"cargo clippy failed for artifact {}: {}",
				artifact.name,
				artifact.path.display()
			);
			std::process::exit(1);
		}
	}
}
