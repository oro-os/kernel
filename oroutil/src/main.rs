//! CLI for development tasks related to the Oro kernel project.

pub(crate) mod build_plan;
pub(crate) mod cmd;
pub(crate) mod crate_info;
pub(crate) mod util;

use clap::{Parser, builder::TypedValueParser};

/// oroutil: Oro kernel development utility
///
/// This tool provides various utilities for developers
/// working on the Oro Operating System kernel project.
#[derive(Parser, Debug)]
struct Args {
	/// Hidden; a small check to make sure that the user
	/// is not running `cargo run` but instead `cargo oro`.
	#[clap(hide = true, long = "_run_from_cargo_oro")]
	is_from_cargo_oro: bool,
	/// Log verbosely. Specify multiple times for more verbosity.
	#[clap(long, short = 'v', action = clap::ArgAction::Count)]
	pub verbose:       u8,
	/// The command to execute
	#[clap(subcommand)]
	command:           Command,
}

/// Subcommands for the Oro utility
#[derive(Parser, Debug)]
enum Command {
	/// Builds the Oro kernel
	Build(BuildArgs),
	/// Formats all files in the project
	Fmt(FmtArgs),
	/// Runs clippy across all crates
	Clippy(ClippyArgs),
	/// Builds documentation
	Doc(DocArgs),
	/// Displays workspace crate information and categorization
	Info(InfoArgs),
}

/// Arguments for the `fmt` command
#[derive(Parser, Debug)]
pub(crate) struct FmtArgs {
	/// Whether or not to simply check the formatting
	#[clap(long, short = 'c')]
	pub check: bool,
}

/// Arguments for the `clippy` command
#[derive(Parser, Debug)]
pub(crate) struct ClippyArgs {
	/// Build configuration arguments.
	#[clap(flatten)]
	pub config: BuildConfig,

	/// Additional arguments to pass to clippy
	#[clap(last = true)]
	pub clippy_args: Vec<String>,
}

/// Arguments for the `doc` command
#[derive(Parser, Debug)]
pub(crate) struct DocArgs {
	/// Build configuration arguments.
	#[clap(flatten)]
	pub config: BuildConfig,

	/// Additional arguments to pass to rustdoc
	#[clap(last = true)]
	pub doc_args: Vec<String>,
}

/// Arguments for the `info` command
#[derive(Parser, Debug)]
pub(crate) struct InfoArgs {}

/// Common arguments for build-like commands
#[derive(Parser, Debug)]
pub(crate) struct BuildConfig {
	/// Build with the given profile(s).
	#[clap(
		long, short='P', value_delimiter = ',',
		default_value = "dev",
		value_parser = clap::builder::PossibleValuesParser::new([
			"dev", "release", "test", "relwithdebinfo",
			"d", "r", "t", "rd"
		]).map(|s| s.parse::<Profile>().unwrap())
	)]
	pub profile: Vec<Profile>,

	/// Build for the given target architecture(s).
	/// If not specified, defaults to all configured targets.
	#[clap(long, short = 't', value_delimiter = ',')]
	pub target: Vec<String>,

	/// Which component types to build (e.g., kernel, limine).
	#[clap(
		long,
		short = 'c',
		value_delimiter = ',',
		default_value = "kernel,limine"
	)]
	pub component: Vec<String>,

	/// Only run one build task at a time.
	#[clap(long, short = 's')]
	pub single_threaded: bool,

	/// Print commands that would be run without executing them.
	#[clap(long)]
	pub dry_run: bool,
}

impl BuildConfig {
	/// Returns the effective list of targets, using all workspace targets if none specified.
	pub fn effective_targets(&self, workspace: &crate_info::WorkspaceCrates) -> Vec<String> {
		if self.target.is_empty() {
			workspace
				.workspace_metadata
				.target
				.keys()
				.cloned()
				.collect()
		} else {
			self.target.clone()
		}
	}
}

/// Arguments for the `build` command
#[derive(Parser, Debug)]
pub(crate) struct BuildArgs {
	/// Build configuration arguments.
	#[clap(flatten)]
	pub config: BuildConfig,
}

/// Profiles usable for building the Oro kernel.
#[derive(Parser, Debug, Clone, PartialEq, Eq, Copy, strum::EnumString, strum::Display)]
pub(crate) enum Profile {
	/// Build with the `dev` profile.
	#[strum(serialize = "dev", serialize = "d")]
	Dev,
	/// Build with the `test` profile.
	#[strum(serialize = "test", serialize = "t")]
	Test,
	/// Build with the `release` profile.
	#[strum(serialize = "release", serialize = "r")]
	Release,
	/// Build with the `relwithdebinfo` profile.
	#[strum(serialize = "relwithdebinfo", serialize = "rd")]
	RelWithDebInfo,
}

fn pmain() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	let verbosity = match args.verbose {
		0 => log::LevelFilter::Info,
		1 => log::LevelFilter::Debug,
		_ => log::LevelFilter::Trace,
	};

	log::set_max_level(verbosity);
	let logger = colog::default_builder().filter_level(verbosity).build();

	if !args.is_from_cargo_oro {
		return Err("`cargo run` does nothing; run from `cargo oro` instead".into());
	}

	match args.command {
		Command::Build(args) => {
			cmd::build::run(args, logger)?;
		}
		Command::Fmt(args) => {
			cmd::fmt::run(args)?;
		}
		Command::Clippy(args) => {
			cmd::clippy::run(args, logger)?;
		}
		Command::Doc(args) => {
			cmd::doc::run(args, logger)?;
		}
		Command::Info(args) => {
			cmd::info::run(args)?;
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
