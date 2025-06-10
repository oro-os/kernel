//! Implements the whole-crate formatter.

pub fn run(args: crate::FmtArgs) -> Result<(), Box<dyn std::error::Error>> {
	// First, shell out to the normal rustfmt command via cargo.
	let mut cmd = crate::util::cargo_command();

	let mut success = true;
	cmd.arg("fmt").arg("--all");

	if args.check {
		let output = cmd
			.arg("--")
			.arg("--emit=json")
			.stdout(std::process::Stdio::piped())
			.spawn()?
			.wait_with_output()?;

		if !output.status.success() {
			return Err(format!("`cargo fmt` failed with status: {}", output.status).into());
		}

		let json_output = String::from_utf8(output.stdout)?;

		if !json_output.is_empty() {
			#[derive(serde::Deserialize)]
			struct FmtOutput {
				name:       String,
				mismatches: Vec<RustMismatch>,
			}

			#[derive(serde::Deserialize)]
			struct RustMismatch {
				original: String,
				expected: String,
			}

			let result = serde_json::from_str::<Vec<FmtOutput>>(&json_output)
				.map_err(|e| format!("failed to parse `cargo fmt` output as JSON: {}", e))?;

			success = success && result.is_empty();

			for unformatted in result {
				log::error!("file is not formatted correctly: {}", unformatted.name);
				for mismatch in unformatted.mismatches {
					let original = mismatch.original.trim_end_matches('\n');
					let expected = mismatch.expected.trim_end_matches('\n');
					if original != expected {
						println!("\x1b[31m-{}\x1b[m", original);
						println!("\x1b[32m+{}\x1b[m", expected);
					}
				}
			}
		}
	} else {
		let status = cmd.spawn()?.wait()?;
		if !status.success() {
			return Err(format!("`cargo fmt` failed with status: {}", status).into());
		}
	}

	// Then find all toml files in the project and format them.
	for toml_file in crate::util::glob_files(&["**/*.toml"])? {
		let contents = std::fs::read_to_string(&toml_file)?;

		let formatted = format_toml(&contents);

		if formatted != contents {
			if args.check {
				log::error!("file is not formatted correctly: {}", toml_file.display());
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
		log::error!("some files are not formatted correctly, run `cargo oro fmt` to fix them");
		std::process::exit(1);
	}

	Ok(())
}

fn format_toml(source: &str) -> String {
	taplo::formatter::format(
		source,
		taplo::formatter::Options {
			align_entries: true,
			align_comments: false,
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
