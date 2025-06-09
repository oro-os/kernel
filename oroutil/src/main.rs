//! CLI for development tasks related to the Oro kernel project.

pub(crate) mod cmd;
pub(crate) mod util;

use clap::Parser;

/// oroutil: Oro kernel development utility
///
/// This tool provides various utilities for developers
/// working on the Oro Operating System kernel project.
#[derive(Parser, Debug)]
struct Args {
	/// The command to execute
	#[clap(subcommand)]
	command: Command,
}

/// Subcommands for the Oro utility
#[derive(Parser, Debug)]
enum Command {
	/// Formats all files in the project.
	Fmt(FmtArgs),
}

/// Arguments for the `fmt` command
#[derive(Parser, Debug)]
pub(crate) struct FmtArgs {
	/// Whether or not to simply check the formatting
	#[clap(long, short = 'c')]
	check: bool,
}

fn pmain() -> Result<(), Box<dyn std::error::Error>> {
	colog::init();

	let args = Args::parse();

	match args.command {
		Command::Fmt(args) => {
			cmd::fmt::run(args)?;
		}
	}

	Ok(())
}

fn main() {
	if let Err(e) = pmain() {
		log::error!("fatal: {}", e);
		std::process::exit(1);
	}
}
