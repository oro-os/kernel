use std::{
	hash::{Hash, Hasher},
	io::{IsTerminal, Read, Seek, Write},
};

use clap::Parser;

#[derive(Parser)]
pub struct Args {
	/// The path to the vendor source file to patch, relative to the workspace root.
	#[arg(value_name = "FILE")]
	file: String,
}

pub fn run(args: Args) {
	let file = std::path::PathBuf::from(args.file);
	if !file.starts_with("vendor/") {
		panic!("error: file must be under vendor/");
	}

	let workspace_root = crate::vfs::Vfs::new_from_cargo().root();
	let file = workspace_root.join(file);

	if !file.exists() {
		panic!("error: file does not exist: {}", file.display());
	}

	let stdin = std::io::stdin();
	if !stdin.is_terminal() {
		panic!("error: patch command only works in a TTY session; do not try to script it!");
	}

	let relpath = pathdiff::diff_paths(&file, workspace_root.join("vendor"))
		.expect("failed to compute relative path")
		.with_added_extension("patch")
		.display()
		.to_string()
		.replace(std::path::MAIN_SEPARATOR, "++");

	let target_path = workspace_root.join(".vendor-patches").join(relpath);
	let target_dir = target_path
		.parent()
		.expect("failed to get parent directory of target path");
	std::fs::create_dir_all(target_dir).expect("failed to create target directory");

	let mut patchfile = std::fs::OpenOptions::new()
		.create(true)
		.write(true)
		.read(true)
		.append(true)
		.open(&target_path)
		.expect("failed to create/open patch file");

	patchfile
		.seek(std::io::SeekFrom::Start(0))
		.expect("failed to seek patch file");

	let mut patch_contents = String::new();
	patchfile
		.read_to_string(&mut patch_contents)
		.expect("failed to read patch file");

	let patch = match flickzeug::Diff::from_str(&patch_contents) {
		Ok(p) => Some(p),
		Err(flickzeug::ParsePatchError::NoHunks) => None,
		Err(err) => {
			panic!("failed to parse patch file: {err}");
		}
	};

	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	target_path.hash(&mut hasher);
	let mut tmpfile = std::env::temp_dir().join(format!("oro-kernel-patch-{}", hasher.finish()));

	if let Some(ext) = file.extension() {
		tmpfile.set_extension(ext);
	}

	let maybe_prepatch_file = file.with_added_extension("_oro-prepatch");
	let original_contents = if maybe_prepatch_file.exists() {
		std::fs::read_to_string(&maybe_prepatch_file).expect("failed to read prepatch file")
	} else {
		std::fs::copy(&file, &maybe_prepatch_file).expect("failed to create prepatch file");
		std::fs::read_to_string(&file).expect("failed to read original file")
	};

	let contents = if let Some(patch) = patch {
		let (contents, _) =
			flickzeug::apply(&original_contents, &patch).expect("failed to apply existing patch");
		contents
	} else {
		original_contents.clone()
	};

	std::fs::write(&tmpfile, contents.as_bytes())
		.expect("failed to write contents to temporary file");

	let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
	let mut cmd = std::process::Command::new(editor);
	cmd.arg(&tmpfile)
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.stdin(std::process::Stdio::inherit());
	let status = cmd.status().expect("failed to launch editor");
	if !status.success() {
		panic!("editor exited with non-zero status; patch file will remain");
	}

	let new_contents = std::fs::read_to_string(&tmpfile).expect("failed to read modified file");
	std::fs::remove_file(tmpfile).expect("failed to remove temporary file");
	let new_patch = flickzeug::create_patch(&original_contents, &new_contents);

	patchfile.set_len(0).expect("failed to truncate patch file");
	patchfile
		.seek(std::io::SeekFrom::Start(0))
		.expect("failed to seek patch file");
	patchfile
		.write_all(&new_patch.to_bytes())
		.expect("failed to write new patch to patch file");

	std::fs::write(&file, new_contents.as_bytes())
		.expect("failed to write modified contents to target file");
}

pub fn apply_all() {
	let workspace_root = crate::vfs::Vfs::new_from_cargo().root();
	let patches_dir = workspace_root.join(".vendor-patches");
	if !patches_dir.exists() {
		return;
	}

	for entry in std::fs::read_dir(&patches_dir).expect("failed to read patch directory") {
		let entry = entry.expect("failed to read patch directory entry");
		let patch_path = entry.path();
		if patch_path.is_file() {
			if patch_path.extension().is_none_or(|ext| ext != "patch") {
				continue;
			}

			let file_stem = patch_path
				.file_stem()
				.expect("failed to get patch file stem")
				.to_string_lossy();

			let file_stem = file_stem.replace("++", &std::path::MAIN_SEPARATOR.to_string());
			let target_path = workspace_root.join("vendor").join(file_stem);

			if !target_path.exists() {
				panic!(
					"patch file {} does not correspond to an existing vendor file",
					patch_path.display()
				);
			}

			eprintln!("applying patch: {}", patch_path.display());
			let patch_contents =
				std::fs::read_to_string(&patch_path).expect("failed to read patch file");
			let patch = match flickzeug::Diff::from_str(&patch_contents) {
				Ok(p) => p,
				Err(flickzeug::ParsePatchError::NoHunks) => continue,
				Err(err) => panic!("failed to parse patch file {}: {err}", patch_path.display()),
			};

			let original_contents =
				std::fs::read_to_string(&target_path).expect("failed to read target file");
			let (new_contents, _) =
				flickzeug::apply(&original_contents, &patch).expect("failed to apply patch");

			std::fs::write(&target_path, new_contents.as_bytes())
				.expect("failed to write patched contents to target file");
		}
	}
}

pub fn check_all() {
	let workspace_root = crate::vfs::Vfs::new_from_cargo().root();
	let patches_dir = workspace_root.join(".vendor-patches");
	if !patches_dir.exists() {
		return;
	}

	let mut ok = true;

	for entry in std::fs::read_dir(&patches_dir).expect("failed to read patch directory") {
		let entry = entry.expect("failed to read patch directory entry");
		let patch_path = entry.path();
		if patch_path.is_file() {
			if patch_path.extension().is_none_or(|ext| ext != "patch") {
				continue;
			}

			let file_stem = patch_path
				.file_stem()
				.expect("failed to get patch file stem")
				.to_string_lossy();

			let file_stem = file_stem.replace("++", &std::path::MAIN_SEPARATOR.to_string());
			let target_path = workspace_root.join("vendor").join(file_stem);

			if !target_path.exists() {
				panic!(
					"patch file {} does not correspond to an existing vendor file",
					patch_path.display()
				);
			}

			let patch_contents =
				std::fs::read_to_string(&patch_path).expect("failed to read patch file");
			let patch = match flickzeug::Diff::from_str(&patch_contents) {
				Ok(p) => p,
				Err(flickzeug::ParsePatchError::NoHunks) => continue,
				Err(err) => panic!("failed to parse patch file {}: {err}", patch_path.display()),
			};

			let patch = patch.reverse();

			let patched_contents =
				std::fs::read_to_string(&target_path).expect("failed to read target file");
			if let Err(err) = flickzeug::apply(&patched_contents, &patch) {
				eprintln!(
					"failed to apply reversed patch {}: {err}",
					patch_path.display()
				);
				ok = false;
			}
		}
	}

	if !ok {
		std::process::exit(1);
	}

	eprintln!("all patches OK");
}
