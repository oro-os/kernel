use std::collections::{HashMap, HashSet};

use clap::Parser;

#[derive(Parser)]
pub struct Args {
	/// Checks that the license files are correct.
	#[arg(long)]
	check: bool,
}

pub fn run(args: Args) {
	let vfs = crate::vfs::Vfs::new_from_cargo();
	let root_cargo_toml = vfs.root_cargo_toml();

	let expected_licenses = vfs.vendor_packages();
	let expected_licenses = if let Some(md) = root_cargo_toml
		.package
		.metadata
		.and_then(|md| md.oro?.license_overrides)
	{
		expected_licenses
			.map(|mut l| {
				l.override_license_info(&md);
				l
			})
			.collect::<Vec<_>>()
	} else {
		expected_licenses.collect::<Vec<_>>()
	};

	let existing_licenses = vfs.vendor_licenses().collect::<Vec<_>>();

	let mut expected_links = HashMap::new();
	let mut missing_licenses = HashSet::new();
	let mut unexpected_links = HashSet::new();

	let canonical_license_dir = std::fs::canonicalize(vfs.vendor_license_dir())
		.expect("failed to canonicalize vendor license directory");

	for expected in expected_licenses {
		if let Some(lf) = expected.license_path {
			let license_link_path = vfs
				.vendor_license_dir()
				.join(expected.manifest.package.name);

			let lf = std::fs::canonicalize(&lf).expect(&format!(
				"failed to canonicalize vendor package license file: {}",
				lf.display()
			));
			let lf = pathdiff::diff_paths(&lf, &canonical_license_dir).expect(&format!(
				"failed to find relative license path: {}",
				lf.display()
			));

			expected_links.insert(
				std::path::absolute(&license_link_path).expect(&format!(
					"failed to absolutize path: {}",
					license_link_path.display()
				)).normalize_lexically().unwrap(),
				lf,
			);
		} else {
			missing_licenses.insert(expected.manifest.package.name);
		}
	}



	for existing in existing_licenses {
		let license_path = existing.license_path.normalize_lexically().unwrap();
		if let Some(expected) = expected_links.remove(&license_path) {
			let relpath = pathdiff::diff_paths(&existing.target_path, &canonical_license_dir).expect(&format!(
				"failed to relativize an existing license path: {}", existing.target_path.display()
			));

			if expected != relpath {
				unexpected_links.insert(license_path.clone());
				// Add it back; we might need to create it.
				expected_links.insert(license_path, expected);
			}
		} else {
			unexpected_links.insert(license_path);
		}
	}

	let mut ok = true;

	for missing in missing_licenses {
		ok = false;
		eprintln!("error: vendor package license file could not be found: {missing}");
	}

	if args.check {
		for (_, link) in expected_links {
			ok = false;
			eprintln!("error: missing vendor license link: {}", link.display());
		}

		for unexpected in unexpected_links {
			ok = false;
			eprintln!("error: unexpected vendor license link: {}", unexpected.display());
		}
	} else {
		for unexpected in unexpected_links {
			eprintln!("remove: {}", unexpected.display());
			std::fs::remove_file(&unexpected).expect("failed to unlink unexpected license link");
		}

		for (from, to) in expected_links {
			eprintln!("link: {} -> {}", from.display(), to.display());
			std::os::unix::fs::symlink(to, from).expect("failed to symlink license file");
		}
	}

	if !ok {
		std::process::exit(1);
	}
}
