pub mod artifact;
pub mod cmd;
pub mod vfs;

use clap::{Parser, Subcommand};

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
}

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::Build(args) => cmd::build::run(args),
		Commands::Clippy(args) => cmd::clippy::run(args),
		Commands::Check(args) => cmd::check::run(args),
		Commands::Vendor(args) => cmd::vendor::run(args),
		Commands::Udeps => cmd::udeps::run(),
	}
}
