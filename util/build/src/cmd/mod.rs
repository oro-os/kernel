pub mod build;
pub mod check;
pub mod clippy;
#[cfg(target_family = "unix")]
pub mod license;
pub mod patch;
pub mod pkg;
pub mod udeps;
pub mod vendor;

pub fn cargo() -> std::process::Command {
	let cargo_command = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
	let mut cmd = std::process::Command::new(cargo_command);

	cmd.env_clear()
		.envs(std::env::vars().filter(|(k, _)| {
			matches!(
				k.as_str(),
				"CARGO_TARGET_DIR" | "CARGO_HOME" | "PATH" | "RUSTUP_HOME" | "RUSTUP_TOOLCHAIN"
			)
		}))
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.stdin(std::process::Stdio::inherit());

	cmd
}
