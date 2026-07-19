use core::hash::Hash;
use std::{collections::HashSet, io::Write};

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

	let mut seen = HashSet::new();

	let mut ok = true;
	for artifact in artifacts {
		let mut cmd = super::cargo();
		cmd
			.arg("check")
			// TODO: reenable after fix: https://github.com/rust-lang/cargo/issues/7058
			//.arg("--frozen")
			.current_dir(&artifact.path);

		if let Some(profile) = &args.profile {
			cmd.arg("--profile").arg(profile);
		}

		let mut dedupe_stdout = false;

		if args.json {
			cmd.arg("--message-format")
				.arg("json-diagnostic-rendered-ansi")
				.arg("--color")
				.arg("never")
				.arg("--quiet")
				.stderr(std::process::Stdio::null())
				.stdout(std::process::Stdio::piped());

			dedupe_stdout = true;
		} else {
			cmd.arg("--color").arg("always");
		}

		let mut child = cmd.spawn().expect(&format!(
			"failed to execute cargo check for artifact {}: {}",
			artifact.name,
			artifact.path.display()
		));

		if dedupe_stdout {
			let stdout = child.stdout.take().expect("failed to capture stdout");
			let mut dedupe_stream = crate::dedupe::DedupeStream::new(&mut seen, std::io::stdout());
			std::io::copy(&mut std::io::BufReader::new(stdout), &mut dedupe_stream)
				.expect("failed to copy stdout");
		}

		let status = child.wait().expect(&format!(
			"failed to execute cargo check for artifact {}: {}",
			artifact.name,
			artifact.path.display()
		));

		std::io::stdout().flush().ok();
		std::io::stderr().flush().ok();

		if !status.success() {
			if !args.json {
				eprintln!(
					"cargo clippy failed for artifact {}: {}",
					artifact.name,
					artifact.path.display()
				);
			}
			ok = false;
		}
	}

	if !ok {
		std::process::exit(1);
	}
}
