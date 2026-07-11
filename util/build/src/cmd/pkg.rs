use std::io::Write;

use clap::Parser;

#[derive(Parser)]
pub struct Args {
	/// Additional parameters to pass to the `pkg` utility.
	///
	/// See `cargo pkg --help` for more information.
	args: Vec<String>,
}

pub fn run(args: Args) {
	// Build the entirety of the kernel first.
	let status = super::cargo()
		.arg("oro")
		.arg("build")
		.status()
		.expect("failed to run cargo build");

	std::io::stdout().flush().ok();
	std::io::stderr().flush().ok();

	if !status.success() {
		std::process::exit(1);
	}

	// Run the `pkg` utility with the provided arguments.
	let status = super::cargo()
		.arg("pkg")
		.args(args.args)
		.status()
		.expect("failed to run cargo pkg");

	std::io::stdout().flush().ok();
	std::io::stderr().flush().ok();

	if !status.success() {
		std::process::exit(1);
	}
}
