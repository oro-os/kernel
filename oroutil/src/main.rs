//! CLI for development tasks related to the Oro kernel project.

pub(crate) mod build_plan;
pub(crate) mod cmd;
pub(crate) mod util;

use clap::{Parser, builder::TypedValueParser};

#[cfg(not(any(test, feature = "run-from-cargo")))]
compile_error!("the Oro kernel cannot be built with `cargo build`; see `cargo oro build --help`");

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
	/// Builds the Oro kernel.
	Build(BuildArgs),
	/// Formats all files in the project.
	Fmt(FmtArgs),
}

/// Arguments for the `fmt` command
#[derive(Parser, Debug)]
pub(crate) struct FmtArgs {
	/// Whether or not to simply check the formatting
	#[clap(long, short = 'c')]
	pub check: bool,
}

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
	#[clap(
		long, short = 't', value_delimiter = ',',
		default_value = "x86_64,aarch64",
		value_parser = clap::builder::PossibleValuesParser::new([
			"x86_64", "aarch64"
		]).map(|s| s.parse::<TargetArch>().unwrap())
	)]
	pub target: Vec<TargetArch>,

	/// Which components to build.
	#[clap(
		long, short = 'c', value_delimiter = ',',
		default_value = "kernel,limine",
		value_parser = clap::builder::PossibleValuesParser::new([
			"kernel", "limine"
		]).map(|s| s.parse::<Component>().unwrap())
	)]
	pub component: Vec<Component>,

	/// Only run one build task at a time.
	#[clap(long, short = 's')]
	pub single_threaded: bool,
}

impl BuildConfig {
	/// Returns a matrix of all combinations of profiles, targets, and components.
	pub fn matrix(&self) -> Vec<(Profile, TargetArch, Component)> {
		let mut matrix = Vec::new();
		for &profile in &self.profile {
			for &target in &self.target {
				for &component in &self.component {
					matrix.push((profile, target, component));
				}
			}
		}
		matrix
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

/// Target architectures for the Oro kernel.
#[derive(Parser, Debug, Clone, PartialEq, Eq, Copy, strum::EnumString, strum::Display)]
pub(crate) enum TargetArch {
	/// Build for the x86_64 architecture.
	#[strum(serialize = "x86_64")]
	X86_64,
	/// Build for the aarch64 architecture.
	#[strum(serialize = "aarch64")]
	Aarch64,
}

impl TargetArch {
	/// Returns the path to the target JSON file.
	pub fn target_json_path(&self) -> &'static str {
		match self {
			TargetArch::X86_64 => "oro-kernel-arch-x86_64/x86_64-unknown-oro.json",
			TargetArch::Aarch64 => "oro-kernel-arch-aarch64/aarch64-unknown-oro.json",
		}
	}
}

/// Components that can be built.
#[derive(Parser, Debug, Clone, PartialEq, Eq, Copy, strum::EnumString, strum::Display)]
pub(crate) enum Component {
	/// The kernel itself.
	#[strum(serialize = "kernel")]
	Kernel,
	/// Limine bootloader.
	#[strum(serialize = "limine")]
	Limine,
}

impl Component {
	/// Returns the crate name for the component and the given architecture.
	pub fn crate_name(&self, arch: TargetArch) -> &'static str {
		match self {
			Component::Kernel => {
				match arch {
					TargetArch::X86_64 => "oro-kernel-arch-x86_64",
					TargetArch::Aarch64 => "oro-kernel-arch-aarch64",
				}
			}
			Component::Limine => "oro-bootloader-limine",
		}
	}

	/// Returns the `--bin` argument for the component and the given architecture,
	/// if applicable.
	pub fn bin_arg(&self, arch: TargetArch) -> Option<String> {
		match self {
			Component::Kernel => None,
			Component::Limine => {
				match arch {
					TargetArch::X86_64 => Some("oro-limine-x86_64".to_string()),
					TargetArch::Aarch64 => Some("oro-limine-aarch64".to_string()),
				}
			}
		}
	}
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
	}

	Ok(())
}

fn main() {
	if let Err(e) = pmain() {
		log::error!("fatal: {}", e);
		std::process::exit(1);
	}
}
