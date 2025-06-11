//! Implements the main build scripts for the Oro kernel.

use std::{
	path::PathBuf,
	process::Command,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering::Relaxed},
	},
};

use cargo_metadata::{Message, diagnostic::DiagnosticLevel};
use indicatif::{MultiProgress, ProgressBar, ProgressFinish, ProgressStyle};
use indicatif_log_bridge::LogWrapper;

use crate::build_plan::BuildPlan;

pub fn run(
	args: crate::BuildArgs,
	logger: impl log::Log + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
	let mp = MultiProgress::new();

	LogWrapper::new(mp.clone(), logger).try_init()?;

	struct BuildMatrix {
		profile:   crate::Profile,
		arch:      crate::TargetArch,
		component: crate::Component,
		pb:        ProgressBar,
		skip:      Arc<AtomicBool>,
	}

	let mut matrix = args
		.config
		.matrix()
		.into_iter()
		.map(|(profile, arch, component)| {
			BuildMatrix {
				profile,
				arch,
				component,
				pb: {
					let pb = mp.add(
						ProgressBar::new_spinner()
							.with_prefix(format!("{} {} {}", component, arch, profile))
							.with_finish(ProgressFinish::AndLeave),
					);
					pb.set_style(
						ProgressStyle::default_bar()
							.template(
								"{spinner:.yellow}   [{elapsed_precise:.dim}] [{prefix:.yellow}] \
								 {msg}",
							)
							.unwrap(),
					);
					pb.set_message("waiting...");
					pb
				},

				skip: Arc::new(AtomicBool::new(false)),
			}
		})
		.collect::<Vec<_>>();

	let success = Arc::new(AtomicBool::new(true));

	for BuildMatrix {
		profile,
		arch,
		component,
		pb,
		skip,
	} in matrix.iter_mut()
	{
		pb.set_message("compiling build plan...");

		let mut cmd = make_program(
			*profile,
			*arch,
			*component,
			Options {
				json:          false,
				target_suffix: Some("plan".into()),
			},
		);

		cmd.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped());

		cmd.arg("--build-plan");

		log::trace!("{cmd:?}");

		let output = cmd.spawn()?.wait_with_output()?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			log::error!(
				"failed to compile build plan for {component} on {arch} with profile {profile}: \
				 {stderr}"
			);
			pb.set_style(
				ProgressStyle::default_bar()
					.template("{spinner:.red}   [{elapsed_precise:.dim}] [{prefix:.red}] {msg}")
					.unwrap(),
			);
			pb.set_message("FAIL");
			pb.finish_using_style();

			skip.store(true, Relaxed);
			continue;
		}

		let plan: BuildPlan = match serde_json::from_slice(&output.stdout) {
			Ok(plan) => plan,
			Err(e) => {
				log::error!(
					"failed to parse build plan for {component} on {arch} with profile {profile}: \
					 {e}"
				);
				pb.set_style(
					ProgressStyle::default_bar()
						.template("{spinner:.red}   [{elapsed_precise:.dim}] [{prefix:.red}] {msg}")
						.unwrap(),
				);
				pb.set_message("FAIL");
				pb.finish_using_style();
				success.store(false, Relaxed);
				skip.store(true, Relaxed);
				continue;
			}
		};

		pb.set_length(plan.invocations.len() as u64);

		pb.set_message("waiting...");
		pb.reset_elapsed();
		pb.set_position(0);
		pb.set_style(
			ProgressStyle::default_bar()
				.template(
					"{spinner:.cyan}   [{elapsed_precise:.dim}] [{prefix:.yellow}] {wide_msg} \
					 {bar:40} {pos:>4}/{len:4}",
				)
				.unwrap()
				.progress_chars("->."),
		);
		pb.tick();
	}

	let mut join_handles = vec![];

	for BuildMatrix {
		profile,
		arch,
		component,
		pb,
		skip,
	} in &matrix
	{
		if skip.load(Relaxed) {
			log::debug!(
				"skipping build for {component} on {arch} with profile {profile} (build plan \
				 failed)"
			);
			continue;
		}

		pb.set_message("building...");

		let crate_name = component.crate_name(*arch);

		log::debug!(
			"building {component} for arch {arch} with profile {profile} (crate: {crate_name})"
		);

		let mut cmd = make_program(
			*profile,
			*arch,
			*component,
			Options {
				json:          true,
				target_suffix: None,
			},
		);
		cmd.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::null());

		log::trace!("{cmd:?}");

		let join_handle = std::thread::spawn({
			let pb = pb.clone();
			let success = success.clone();

			move || -> Result<(), std::io::Error> {
				let mut cmd = cmd.spawn()?;

				let reader = std::io::BufReader::new(cmd.stdout.take().unwrap());

				let mut build_success = true;

				for message in Message::parse_stream(reader) {
					match message? {
						Message::CompilerMessage(msg) => {
							match msg.message.level {
								DiagnosticLevel::Error | DiagnosticLevel::Ice => {
									if let Some(rendered) = &msg.message.rendered {
										log::error!("{}", rendered);
									}
									build_success = false;
								}
								DiagnosticLevel::Warning => {
									if let Some(rendered) = &msg.message.rendered {
										log::warn!("{}", rendered);
									}
								}
								_ => {
									if let Some(rendered) = &msg.message.rendered {
										log::info!("{}", rendered);
									}
								}
							}
						}
						Message::CompilerArtifact(artifact) => {
							pb.inc(1);
							log::debug!("built artifact: {}", artifact.target.name);
						}
						Message::BuildScriptExecuted(script) => {
							pb.inc(1);
							log::debug!("build script: {}", script.package_id);
						}
						Message::TextLine(line) => {
							log::info!("{line}");
						}
						Message::BuildFinished(finished) => {
							build_success = build_success && finished.success;
						}
						_ => (), // Unknown message
					}
				}

				build_success = build_success && cmd.wait()?.success();

				if build_success {
					pb.set_style(
						ProgressStyle::default_bar()
							.template(
								"{spinner:.green}   [{elapsed_precise:.dim}] [{prefix:.green}] \
								 {msg}",
							)
							.unwrap(),
					);
					pb.set_message("OK");
				} else {
					pb.set_style(
						ProgressStyle::default_bar()
							.template(
								"{spinner:.red}   [{elapsed_precise:.dim}] [{prefix:.red}] {msg}",
							)
							.unwrap(),
					);
					pb.set_message("FAIL");
					success.store(false, Relaxed);
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
					log::debug!("build completed successfully: {}", pb.prefix());
				}
				Ok(Err(e)) => {
					log::error!("build failed: {}: {e}", pb.prefix());
				}
				Err(e) => {
					log::error!("build thread panicked: {}: {e:?}", pb.prefix());
				}
			}
		}
	}

	for join_handle in join_handles {
		let (handle, pb) = join_handle;
		match handle.join() {
			Ok(Ok(())) => {
				log::debug!("build completed successfully: {}", pb.prefix());
			}
			Ok(Err(e)) => {
				log::error!("build failed: {}: {e}", pb.prefix());
			}
			Err(e) => {
				log::error!("build thread panicked: {}: {e:?}", pb.prefix());
			}
		}
	}

	if success.load(Relaxed) {
		Ok(())
	} else {
		Err("some builds failed".into())
	}
}

struct Options {
	json:          bool,
	target_suffix: Option<String>,
}

fn make_program(
	profile: crate::Profile,
	arch: crate::TargetArch,
	component: crate::Component,
	options: Options,
) -> Command {
	let base_target_dir =
		PathBuf::from(std::env::var("CARGO_TARGET_DIR").expect("CARGO_TARGET_DIR not set"));

	let target_dir_basename = {
		let mut basename = format!(
			"{}-{}-{}",
			component.crate_name(arch),
			profile.to_string(),
			arch.to_string(),
		);
		if let Some(suffix) = options.target_suffix {
			basename.push_str(&format!("-{suffix}"));
		}
		basename
	};

	let target_dir = base_target_dir.join(target_dir_basename);

	let mut cmd = crate::util::cargo_command();
	cmd.arg("build")
		.arg("--quiet")
		.arg("--keep-going")
		.arg("--target")
		.arg(arch.target_json_path())
		.arg("--profile")
		.arg(profile.to_string())
		.arg("--package")
		.arg(component.crate_name(arch))
		.env("CARGO_TARGET_DIR", target_dir.display().to_string());

	if options.json {
		cmd.arg("--message-format").arg("json");
	}

	if let Some(bin_arg) = component.bin_arg(arch) {
		cmd.arg("--bin").arg(bin_arg);
	}

	cmd.arg("-Zunstable-options")
		.arg("-Zbuild-std=core,compiler_builtins,alloc")
		.arg("-Zbuild-std-features=compiler-builtins-mem");

	cmd
}
