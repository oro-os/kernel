//! Implements clippy command for the Oro kernel.

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;

use crate::crate_info::WorkspaceCrates;

pub fn run(
	args: crate::ClippyArgs,
	logger: impl log::Log + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
	let mp = MultiProgress::new();
	LogWrapper::new(mp.clone(), logger).try_init()?;

	let workspace = WorkspaceCrates::load()?;

	let mut tasks = Vec::new();

	// Add pass for crates that build for host
	let host_crates = workspace.host_crates();
	if !host_crates.is_empty() {
		for &profile in &args.config.profile {
			let pb = mp.add(ProgressBar::new_spinner());
			pb.set_style(
				ProgressStyle::default_spinner()
					.template("{spinner:.cyan} [{elapsed_precise:.dim}] [{prefix:.yellow}] {msg}")
					.unwrap(),
			);
			pb.set_prefix(format!("clippy host {}", profile));
			pb.set_message("running...");

			tasks.push((profile, None, host_crates.clone(), pb));
		}
	}

	// Build a matrix of (profile, arch, crates) for arch-specific builds
	for &profile in &args.config.profile {
		for &arch in &args.config.target {
			let arch_str = arch.to_string();
			let crates_for_arch = workspace.for_arch(&arch_str);

			if crates_for_arch.is_empty() {
				continue;
			}

			let pb = mp.add(ProgressBar::new_spinner());
			pb.set_style(
				ProgressStyle::default_spinner()
					.template("{spinner:.cyan} [{elapsed_precise:.dim}] [{prefix:.yellow}] {msg}")
					.unwrap(),
			);
			pb.set_prefix(format!("clippy {} {}", arch, profile));
			pb.set_message("running...");

			tasks.push((profile, Some(arch), crates_for_arch, pb));
		}
	}

	let mut success = true;

	for (profile, arch_opt, crates, pb) in tasks {
		let mut cmd = crate::util::cargo_command();
		cmd.arg("clippy");

		if let Some(arch) = arch_opt {
			cmd.arg("--target").arg(arch.target_json_path());

			// Set unique target directory to avoid locking and cache invalidation
			let base_target_dir =
				std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
			let target_dir = format!("{}/clippy-{}-{}", base_target_dir, arch, profile);
			cmd.env("CARGO_TARGET_DIR", &target_dir);

			// Add architecture-specific features
			let features = match arch {
				crate::TargetArch::X86_64 => "oro-debug/uart16550",
				crate::TargetArch::Aarch64 => "oro-debug/pl011",
			};
			cmd.arg("--features").arg(features);
		}

		cmd.arg("--profile").arg(profile.to_string());

		// Add all crate packages
		for crate_info in &crates {
			cmd.arg("-p").arg(crate_info.package.name.as_str());
		}

		// Check if we need build-std for any of these crates (no_std = true)
		let needs_build_std = crates
			.iter()
			.any(|c| c.oro_metadata.no_std.unwrap_or(false));

		if needs_build_std {
			cmd.arg("-Zunstable-options")
				.arg("-Zbuild-std=core,compiler_builtins,alloc")
				.arg("-Zbuild-std-features=compiler-builtins-mem");
		}

		// Add any additional clippy args
		if !args.clippy_args.is_empty() {
			cmd.arg("--");
			cmd.args(&args.clippy_args);
		}

		log::debug!("running: {cmd:?}");

		let output = cmd.output()?;

		if output.status.success() {
			pb.set_style(
				ProgressStyle::default_spinner()
					.template("{spinner:.green} [{elapsed_precise:.dim}] [{prefix:.green}] {msg}")
					.unwrap(),
			);
			pb.set_message("OK");
		} else {
			pb.set_style(
				ProgressStyle::default_spinner()
					.template("{spinner:.red} [{elapsed_precise:.dim}] [{prefix:.red}] {msg}")
					.unwrap(),
			);
			pb.set_message("FAIL");
			success = false;

			// Print stderr on failure
			if !output.stderr.is_empty() {
				eprintln!("{}", String::from_utf8_lossy(&output.stderr));
			}
		}

		pb.finish();
	}

	if success {
		Ok(())
	} else {
		Err("clippy failed for some crates".into())
	}
}
