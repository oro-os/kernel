#![feature(normalize_lexically)]

pub mod artifact;
pub mod cmd;
pub mod dedupe;

use clap::{Parser, Subcommand};
pub use orok_util_common::vfs;

/// Oro kernel build utility.
#[derive(Parser)]
#[command(override_usage = "cargo oro <COMMAND>")]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Build the kernel and its artifacts.
	#[command(override_usage = "cargo oro build [OPTIONS] [ARTIFACTS]")]
	Build(cmd::build::Args),
	/// Lint the kernel and its artifacts using Clippy.
	#[command(override_usage = "cargo oro clippy [OPTIONS] [ARTIFACTS]")]
	Clippy(cmd::clippy::Args),
	/// Check the kernel and its artifacts using `cargo check`.
	#[command(override_usage = "cargo oro check [OPTIONS] [ARTIFACTS]")]
	Check(cmd::check::Args),
	/// Vendor sources and update references in `.cargo/config.toml`.
	#[command(override_usage = "cargo oro vendor [OPTIONS] [ARTIFACTS]")]
	Vendor(cmd::vendor::Args),
	/// Checks for unused dependencies.
	#[command(override_usage = "cargo oro udeps")]
	Udeps,
	/// Updates and checks the `license/` directory.
	#[cfg(target_family = "unix")]
	#[command(override_usage = "cargo license [OPTIONS]")]
	License(cmd::license::Args),
	/// Creates a patch for a vendor source file.
	#[command(override_usage = "cargo oro patch [OPTIONS] <vendor/<dependency>/path/to/file.rs>")]
	Patch(cmd::patch::Args),
	/// Packages one or more artifacts into a distributable format.
	///
	/// Arguments are passed directly to the `cargo pkg` utility.
	/// See `cargo pkg --help` for more information.
	#[command(override_usage = "cargo oro pkg [ARGS...]")]
	Pkg(cmd::pkg::Args),
}

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::Build(args) => cmd::build::run(args),
		Commands::Clippy(args) => cmd::clippy::run(args),
		Commands::Check(args) => cmd::check::run(args),
		Commands::Vendor(args) => cmd::vendor::run(args),
		Commands::Udeps => cmd::udeps::run(),
		#[cfg(target_family = "unix")]
		Commands::License(args) => cmd::license::run(args),
		Commands::Patch(args) => cmd::patch::run(args),
		Commands::Pkg(args) => cmd::pkg::run(args),
	}
}
