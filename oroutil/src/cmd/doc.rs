//! Implements doc command for the Oro kernel.

use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering::Relaxed},
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;

use crate::crate_info::WorkspaceCrates;

pub fn run(
	args: crate::DocArgs,
	logger: impl log::Log + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
	let mp = MultiProgress::new();
	LogWrapper::new(mp.clone(), logger).try_init()?;

	let workspace = WorkspaceCrates::load()?;

	let mut tasks = Vec::new();

	// Build docs for each target architecture
	for &arch in &args.config.target {
		// Get crates compatible with the target architecture
		let crates = workspace.for_arch(arch);

		if crates.is_empty() {
			continue;
		}

		let pb = mp.add(ProgressBar::new_spinner());
		pb.set_style(
			ProgressStyle::default_spinner()
				.template("{spinner:.cyan} [{elapsed_precise:.dim}] [{prefix:.yellow}] {msg}")
				.unwrap(),
		);
		pb.set_prefix(format!("doc {}", arch));
		pb.set_message("building...");

		tasks.push((arch, crates, pb));
	}

	let success = Arc::new(AtomicBool::new(true));
	let mut join_handles = vec![];

	for (arch, crates, pb) in tasks {
		log::info!(
			"building docs for {} crates compatible with {}",
			crates.len(),
			arch
		);

		let mut cmd = crate::util::cargo_command();
		cmd.arg("doc").arg("--lib").arg("--document-private-items");

		// Set unique target directory to avoid locking and cache invalidation
		let base_target_dir =
			std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
		let target_dir = format!("{}/doc-{}", base_target_dir, arch);
		cmd.env("CARGO_TARGET_DIR", &target_dir);

		// Add architecture-specific features
		let features = match arch {
			crate::TargetArch::X86_64 => "oro-debug/uart16550",
			crate::TargetArch::Aarch64 => "oro-debug/pl011",
		};
		cmd.arg("--features").arg(features);

		// Add target if any crate needs build-std
		let needs_build_std = crates.iter().any(|c| c.oro_metadata.needs_build_std());

		if needs_build_std {
			cmd.arg("--target")
				.arg(arch.target_json_path())
				.arg("-Zunstable-options")
				.arg("-Zbuild-std=core,compiler_builtins,alloc")
				.arg("-Zbuild-std-features=compiler-builtins-mem");
		}

		// Add all compatible crates
		for crate_info in &crates {
			cmd.arg("-p").arg(crate_info.package.name.as_str());
		}

		// Add any additional doc args
		if !args.doc_args.is_empty() {
			cmd.arg("--");
			cmd.args(&args.doc_args);
		}

		cmd.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::piped());

		log::debug!("running: {cmd:?}");

		let join_handle = std::thread::spawn({
			let pb = pb.clone();
			let success = success.clone();

			move || -> Result<(), std::io::Error> {
				let output = cmd.output()?;

				if output.status.success() {
					pb.set_style(
						ProgressStyle::default_spinner()
							.template(
								"{spinner:.green} [{elapsed_precise:.dim}] [{prefix:.green}] {msg}",
							)
							.unwrap(),
					);
					pb.set_message("OK");
				} else {
					pb.set_style(
						ProgressStyle::default_spinner()
							.template(
								"{spinner:.red} [{elapsed_precise:.dim}] [{prefix:.red}] {msg}",
							)
							.unwrap(),
					);
					pb.set_message("FAIL");
					success.store(false, Relaxed);

					// Print stderr on failure
					if !output.stderr.is_empty() {
						eprintln!("{}", String::from_utf8_lossy(&output.stderr));
					}
				}

				pb.finish();

				Ok(())
			}
		});

		if !args.config.single_threaded {
			join_handles.push((join_handle, pb));
		} else {
			match join_handle.join() {
				Ok(Ok(())) => {
					log::debug!("doc build completed successfully: {}", pb.prefix());
				}
				Ok(Err(e)) => {
					log::error!("doc build failed: {}: {e}", pb.prefix());
				}
				Err(e) => {
					log::error!("doc build thread panicked: {}: {e:?}", pb.prefix());
				}
			}
		}
	}

	for join_handle in join_handles {
		let (handle, pb) = join_handle;
		match handle.join() {
			Ok(Ok(())) => {
				log::debug!("doc build completed successfully: {}", pb.prefix());
			}
			Ok(Err(e)) => {
				log::error!("doc build failed: {}: {e}", pb.prefix());
			}
			Err(e) => {
				log::error!("doc build thread panicked: {}: {e:?}", pb.prefix());
			}
		}
	}

	if success.load(Relaxed) {
		Ok(())
	} else {
		Err("doc build failed for some architectures".into())
	}
}
