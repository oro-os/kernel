use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use orok_test_harness::{Packet, State, X8664State};
use tokio::sync::mpsc::Receiver;

use crate::{atomic::RelaxedAtomic, service::session::Arch};

const EVENT_CAP: usize = 128;

pub enum Event {
	Reset { arch: Arch },
	Packet { packet: Packet },
}

struct EventLogger {
	state:      Arc<State>,
	debug_strs: Vec<u8>,
}

impl EventLogger {
	async fn new(state: &Arc<State>, elf_file: Option<impl AsRef<Path>>) -> Result<Self> {
		Ok(Self {
			state:      Arc::clone(state),
			debug_strs: if let Some(elf_file) = elf_file {
				Self::read_elf_dbgstrs(elf_file).await?
			} else {
				b"\0".to_vec()
			},
		})
	}

	fn get_current_location(&self) -> Option<&str> {
		let offset = self.state.last_debug_loc_offset.get();
		if offset == 0 {
			return None;
		}

		let mut pos = offset as usize;
		while pos < self.debug_strs.len() && self.debug_strs[pos] != 0 {
			pos += 1;
		}

		std::str::from_utf8(&self.debug_strs[offset as usize..pos])
			.ok()
			.filter(|s| !s.is_empty())
	}

	async fn read_elf_dbgstrs(elf_file: impl AsRef<Path>) -> Result<Vec<u8>> {
		let elf_data = tokio::fs::read(elf_file)
			.await
			.context("failed to read ELF file for debug strings")?;
		let elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(elf_data.as_slice())
			.context("failed to parse ELF file for debug strings")?;

		let (section_headers, strtab) = elf
			.section_headers_with_strtab()
			.context("failed to read ELF section headers for debug strings")?;
		let (section_headers, strtab) = (
			section_headers.context("failed to find section headers for ELF file")?,
			strtab.context("failed to find section header string table for ELF file")?,
		);
		let debug_section = section_headers
			.iter()
			.find(|sh| {
				strtab
					.get(sh.sh_name as usize)
					.is_ok_and(|name| name == ".orok_test_strings")
			})
			.context("failed to find .debug_str section in ELF file")?;
		let (debug_slice, compression) = elf
			.section_data(&debug_section)
			.context("failed to read .orok_test_strings section data from ELF file")?;

		if compression.is_some() {
			bail!("compressed .orok_test_strings sections are not supported");
		}

		log::debug!("loaded ELF debug strings: {debug_slice:X?}");

		Ok(debug_slice.to_vec())
	}
}

impl orok_test_harness::EventHandler for EventLogger {
	fn handle_event(&self, event: orok_test_harness::Event) {
		let mut core = i64::from(self.state.last_core.get());

		if core >= 255 {
			core = -1;
		}

		if let Some(location) = self.get_current_location() {
			log::warn!("[orok-test] on core {core} at {location}: {event}");
		} else {
			log::warn!("[orok-test] on core {core} at <unknown>: {event}");
		}
	}
}

pub async fn run(bus: Arc<crate::Bus>, mut rx: Receiver<Event>) -> Result<!> {
	let mut state = Arc::new(State::for_arch::<X8664State>());
	let mut logger = EventLogger::new(&state, None::<&'static str>).await?;

	let mut events = Vec::with_capacity(EVENT_CAP);

	loop {
		events.clear();
		let count = rx.recv_many(&mut events, EVENT_CAP).await;
		if count == 0 {
			const {
				// Sanity check here.
				assert!(EVENT_CAP != 0, "EVENT_CAP cannot be 0");
			}

			bail!("EOF");
		}

		for event in &events {
			match event {
				Event::Reset { arch } => {
					// We tell the state machine that it will receive CPU update
					// events since we're using QEMU.
					state = Arc::new(State::for_arch_type((*arch).into()).will_receive_cpu_state());
					logger = EventLogger::new(&state, Some(arch.boot_target_path())).await?;

					bus.tui
						.send(crate::service::tui::Event::SetDebugState {
							debug_state: Arc::clone(&state),
						})
						.await
						.context("failed to send new DebugState to TUI service")?;

					log::debug!("reset orodbg state");
				}
				Event::Packet { packet } => {
					state.handle_packet(packet, &logger);
					crate::service::debounce::invalidate();
				}
			}
		}
	}
}

impl From<Arch> for orok_test_harness::ArchType {
	fn from(value: Arch) -> Self {
		match value {
			Arch::X86_64 => orok_test_harness::ArchType::X8664,
			Arch::Aarch64 => orok_test_harness::ArchType::Aarch64,
			Arch::Riscv64 => orok_test_harness::ArchType::Riscv64,
		}
	}
}
