//! Implements the whole-crate formatter.

use crate::FmtArgs;

pub fn run(args: FmtArgs) -> Result<(), Box<dyn std::error::Error>> {
	// First, shell out to the normal rustfmt command via cargo.
	let mut cmd = crate::util::cargo_command();

	cmd.arg("fmt").arg("--all");

	if args.check {
		cmd.arg("--check");
	}

	let status = cmd.spawn()?.wait()?;

	if !status.success() {
		return Err(format!("`cargo fmt` failed with status: {}", status).into());
	}

	// Then find all toml files in the project and format them.
	let mut success = true;
	for toml_file in crate::util::glob_files(&["**/*.toml"])? {
		let contents = std::fs::read_to_string(&toml_file)?;

		let formatted = format_toml(&contents);

		if formatted != contents {
			if args.check {
				for (change, line) in
					similar::utils::diff_lines(similar::Algorithm::Patience, &contents, &formatted)
				{
					let line = line.trim_end_matches('\n');
					match change {
						similar::ChangeTag::Insert => {
							success = false;
							println!("\x1b[32m+{line}\x1b[m");
						}
						similar::ChangeTag::Delete => {
							success = false;
							println!("\x1b[31m-{line}\x1b[m");
						}
						_ => {}
					}
				}
			} else {
				std::fs::write(&toml_file, formatted)?;
			}
		}
	}

	if !success {
		std::process::exit(1);
	}

	Ok(())
}

fn format_toml(source: &str) -> String {
	taplo::formatter::format(
		source,
		taplo::formatter::Options {
			align_entries: true,
			align_comments: true,
			align_single_comments: true,
			array_trailing_comma: true,
			array_auto_expand: true,
			array_auto_collapse: true,
			inline_table_expand: false,
			compact_inline_tables: false,
			compact_arrays: true,
			allowed_blank_lines: 1,
			compact_entries: false,
			column_width: 50,
			indent_tables: false,
			indent_entries: false,
			indent_string: "\t".into(),
			trailing_newline: true,
			reorder_arrays: true,
			reorder_inline_tables: false,
			reorder_keys: false,
			crlf: false,
		},
	)
}
