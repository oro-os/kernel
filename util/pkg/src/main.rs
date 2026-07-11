pub mod build;
pub mod limine;
pub mod oro;
pub mod registry;

use clap::Parser;

/// Oro kernel packaging utility.
///
/// Crawls the packaging scripts under `pkg/platform`, treating each of
/// their exports as a buildable package artifact, and produces the
/// requested artifacts into `target/pkg`.
#[derive(Parser)]
struct Cli {
	/// The package(s) to build; builds all discovered packages if omitted.
	package: Vec<String>,

	/// List the discovered packages and exit.
	#[clap(long)]
	list: bool,
}

fn main() {
	let cli = Cli::parse();

	let workspace_root = orok_util_common::vfs::Vfs::new_from_cargo().root();
	let workspace_root = std::fs::canonicalize(&workspace_root).unwrap_or(workspace_root);

	let mut registry = registry::Registry::load(&workspace_root).unwrap_or_else(|error| {
		eprintln!("error: {error}");
		std::process::exit(1);
	});

	if cli.list {
		for script in &registry.scripts {
			for (name, _) in &script.exports {
				println!("{name}\t{}", script.path.display());
			}
		}
		return;
	}

	// Resolve the requested package names to their exports, defaulting
	// to everything.
	let selections: Vec<(usize, usize)> = if cli.package.is_empty() {
		registry
			.scripts
			.iter()
			.enumerate()
			.flat_map(|(si, script)| (0..script.exports.len()).map(move |ei| (si, ei)))
			.collect()
	} else {
		cli.package
			.iter()
			.map(|requested| {
				registry
					.scripts
					.iter()
					.enumerate()
					.find_map(|(si, script)| {
						script
							.exports
							.iter()
							.position(|(name, _)| name == requested)
							.map(|ei| (si, ei))
					})
					.unwrap_or_else(|| {
						eprintln!(
							"error: unknown package '{requested}' (use --list to see the \
							 available packages)"
						);
						std::process::exit(1);
					})
			})
			.collect()
	};

	let mut ok = true;

	for (script_index, export_index) in selections {
		let script = &mut registry.scripts[script_index];
		let (name, constructor) = script.exports[export_index].clone();

		match build::build(&mut script.koto, &workspace_root, &name, &constructor) {
			Ok(path) => println!("built {}", path.display()),
			Err(error) => {
				eprintln!("error: {error}");
				ok = false;
			}
		}
	}

	if !ok {
		std::process::exit(1);
	}
}
