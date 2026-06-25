use clap::Parser;

#[derive(Parser)]
struct Cli {
	/// The package(s) to build.
	package: Vec<String>,
}

fn main() {
	let cli = Cli::parse();

	println!("Building packages: {:?}", cli.package);
}
