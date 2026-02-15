use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tokio::sync::mpsc::Receiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
	X86_64,
	Aarch64,
	Riscv64,
}

impl Arch {
	pub fn boot_target_path(&self) -> &'static str {
		match self {
			Self::X86_64 => "target/x86_64-unknown-oro/debug/oro-limine-x86_64",
			Self::Aarch64 => "target/aarch64-unknown-oro/debug/oro-limine-aarch64",
			Self::Riscv64 => "target/riscv64-unknown-oro/debug/oro-limine-riscv64",
		}
	}

	pub fn make_target(&self) -> &'static str {
		match self {
			Self::X86_64 => "run-x86_64",
			Self::Aarch64 => "run-aarch64",
			Self::Riscv64 => "run-riscv64",
		}
	}
}

pub enum Event {
	/// Halts all running session services and disconnects all clients.
	Scram,
	/// Halts all running session services and disconnects all clients, then
	/// exits the program.
	ScramAndQuit { status_code: i32 },
	/// Starts a new session for the given architecture.
	StartSession { arch: Arch },
	/// Tells the session manager that QEMU has connected. This forwards
	/// a message to the GDB service to begin a new debugging session.
	QemuConnected,
}

pub async fn run(bus: Arc<crate::Bus>, mut rx: Receiver<Event>) -> Result<!> {
	let mut current_arch = None;

	while let Some(event) = rx.recv().await {
		match event {
			Event::Scram => {
				current_arch = None;
				scram(&bus).await?;
			}
			Event::ScramAndQuit { status_code } => {
				log::info!("scram and quit - bye!");
				scram(&bus).await?;
				tokio::time::sleep(std::time::Duration::from_millis(200)).await;
				bus.app
					.send(crate::AppEvent::Quit { status_code })
					.await
					.expect("failed to send Quit event to app service");
			}
			Event::StartSession { arch } => {
				scram(&bus).await?;
				log::info!(
					"starting new session for architecture: {arch:?} (`make {}`)",
					arch.make_target()
				);
				current_arch = Some(arch);

				bus.orodbg
					.send(crate::service::orodbg::Event::Reset { arch })
					.await
					.with_context(|| "failed to send Reset event to orodbg service")?;

				bus.mi
					.send(crate::service::mi::Event::Start)
					.await
					.with_context(|| "failed to send Start event to MI service")?;

				bus.qemu
					.send(crate::service::qemu::Event::Start {
						args: vec![arch.make_target().to_string()],
					})
					.await
					.with_context(|| "failed to send StartSession event to qemu service")?;
			}
			Event::QemuConnected => {
				if let Some(arch) = current_arch {
					log::debug!(
						"QEMU connected; starting GDB for target path: {:?}",
						arch.boot_target_path()
					);
					bus.gdb
						.send(crate::service::gdb::Event::Start {
							args: vec![arch.boot_target_path().to_string()],
						})
						.await
						.with_context(|| "failed to send StartSession event to gdb service")?;
				} else {
					log::warn!("QEMU connected but no session is active, telling QEMU to scram");
					bus.qemu
						.send(crate::service::qemu::Event::Scram)
						.await
						.with_context(|| "failed to send scram event to qemu service")?;
				}
			}
		}
	}

	bail!("session service event channel closed unexpectedly");
}

async fn scram(bus: &crate::Bus) -> Result<()> {
	log::debug!("scramming all services");
	bus.video
		.send(crate::service::video::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to video service")?;
	bus.qemu_rsp
		.send(crate::service::qemu_rsp::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to qemu_rsp service")?;
	bus.gdb_rsp
		.send(crate::service::gdb_rsp::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to gdb_rsp service")?;
	bus.gdb
		.send(crate::service::gdb::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to gdb service")?;
	bus.qemu
		.send(crate::service::qemu::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to qemu service")?;
	bus.mi
		.send(crate::service::mi::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to MI service")?;
	bus.orodbg_sock
		.send(crate::service::orodbg_sock::Event::Scram)
		.await
		.with_context(|| "failed to send scram event to Oro kernel debug service")?;

	Ok(())
}
