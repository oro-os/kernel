//! Utilities used throughout the `oroutil` CLI.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// Returns a [`std::process::Command`] for running `cargo`.
///
/// Automatically resolves the `cargo` binary from the current environment.
pub fn cargo_command() -> std::process::Command {
	let cargo_program = std::env::var("CARGO").unwrap_or_else(|_| {
		static HAS_WARNED: AtomicBool = AtomicBool::new(false);
		if !HAS_WARNED.swap(true, Relaxed) {
			log::warn!("`CARGO` environment variable not set; using 'cargo' as default");
		}
		"cargo".to_string()
	});

	let mut cmd = std::process::Command::new(cargo_program);

	cmd.arg("--color=always")
		.env("RUST_BACKTRACE", "1")
		.env("RUST_LOG", "info");

	cmd
}

/// Returns metadata from `cargo metadata` for the current project.
pub fn cargo_metadata<T>() -> Result<T, Box<dyn std::error::Error>>
where
	T: serde::de::DeserializeOwned,
{
	let mut cmd = cargo_command();
	cmd.arg("metadata").arg("--format-version=1");

	let output = cmd.output()?;

	if !output.status.success() {
		return Err(format!("`cargo metadata` failed with status: {}", output.status).into());
	}

	let metadata: T = serde_json::from_slice(&output.stdout)?;
	Ok(metadata)
}

/// Gets the root directory to the repository.
pub fn repo_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
	// Try to find the root by asking Cargo for it.
	#[derive(serde::Deserialize)]
	struct WorkspaceRootMetadata {
		workspace_root: String,
	}
	let maybe_metadata = cargo_metadata::<WorkspaceRootMetadata>();
	if let Ok(maybe_metadata) = maybe_metadata {
		return Ok(std::path::PathBuf::from(maybe_metadata.workspace_root));
	}

	// Otherwise, try to find the root by looking for a `.git` directory.
	let mut path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
	while !path.join(".git").exists() {
		if !path.pop() {
			// If we can't pop any more, we're at the root of the filesystem.
			return Err(
				"could not find repository root: CARGO_MANIFEST_DIR is empty, `cargo metadata` \
				 failed, and no .git directory found"
					.into(),
			);
		}
	}

	Ok(path)
}

/// Returns an iterator over the given file globs in the repository.
///
/// Ignores anything that is ignored by `.gitignore` (including `.git` itself).
pub fn glob_files(
	globs: &[&str],
) -> Result<impl Iterator<Item = std::path::PathBuf>, Box<dyn std::error::Error>> {
	let root_dir = format!("{}/", repo_root()?.to_string_lossy().trim_end_matches('/'));

	Ok(ignore::Walk::new(&root_dir).filter_map(move |entry| {
		if entry.as_ref().is_ok_and(|e| {
			e.file_type().is_some_and(|ft| ft.is_file())
				&& !e.path_is_symlink()
				&& globs.iter().any(|glob| {
					let pth = e.path().to_string_lossy().to_string();
					fast_glob::glob_match(glob, pth.strip_prefix(&root_dir).unwrap_or(&pth))
				})
		}) {
			let Ok(pth) = entry.map(|e| e.into_path()) else {
				return None;
			};
			Some(pth)
		} else {
			None
		}
	}))
}
