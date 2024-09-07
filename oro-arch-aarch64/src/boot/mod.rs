//! Boot routine for the AArch64 architecture.
//!
//! This module prepares the kernel on AArch64
//! directly after being transferred to by the
//! bootloader.

mod memory;
mod protocol;

use core::ffi::CStr;
use oro_boot_protocol::device_tree::{DeviceTreeDataV0, DeviceTreeKind};
use oro_debug::{dbg, dbg_err, dbg_warn};
use oro_dtb::{FdtHeader, FdtPathFilter, FdtToken};
use oro_macro::asm_buffer;
use oro_mem::{pfa::alloc::PageFrameAllocate, translate::Translator};
use oro_type::Be;

/// Boots the primary core on AArch64.
///
/// # Safety
/// Meant only to be called by the entry point.
/// Do not call this directly. It does not reset
/// the kernel or anything else magic like that.
///
/// # Panics
/// Panics if the DeviceTree blob is not provided.
pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();

	#[allow(unused_variables)] // XXX(qix-): Temporary for CI
	let memory::PreparedMemory { mut pfa, pat } = memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(pat.offset());

	let version = crate::psci::PsciMethod::Hvc.psci_version();
	dbg!("PSCI version: {version:?}");

	// Get the devicetree blob.
	let DeviceTreeKind::V0(dtb) = protocol::DTB_REQUEST
		.response()
		.expect("no DeviceTree blob response was provided")
	else {
		panic!("DeviceTree blob response was provided but was the wrong revision");
	};

	let DeviceTreeDataV0 { base, length } = dtb.assume_init_ref();
	dbg!("got DeviceTree blob of {} bytes", length);

	let dtb = FdtHeader::from(pat.translate::<u8>(*base), Some(*length)).expect("dtb is invalid");
	let boot_cpuid = dtb.phys_id();
	dbg!("dtb is valid; primary core id is {boot_cpuid}");

	// XXX
	let phys = pfa.allocate().expect("failed to allocate memory");
	let virt = pat.translate_mut::<[u8; 4096]>(phys);
	#[allow(clippy::missing_docs_in_private_items)]
	const ASMBUF: &[u8] = &asm_buffer!("3:", "wfe", "b 3b",);
	(&mut *virt)[..ASMBUF.len()].copy_from_slice(ASMBUF);

	let mut is_cpu = true; // Assume it is; `device_type` is deprecated.
	let mut reg_val: u64 = 0;
	let mut is_psci = false;
	let mut cpu_id: u64 = 0;
	let mut valid: bool = false;
	for tkn in dtb.iter().filter_path(&[c"", c"cpus", c"cpu@"]) {
		dbg!("     @ {:?}", tkn);
		#[allow(clippy::redundant_guards)] // False positive
		match tkn {
			FdtToken::Node { name } => {
				is_cpu = true;
				reg_val = 0;
				cpu_id = 0;
				valid = true;
				is_psci = false;

				// Extract the text after the last `@` in the `name` string.
				let name = name.to_bytes();
				let Some(idx) = name.iter().rposition(|&c| c == b'@') else {
					dbg_warn!("invalid CPU node name: {:?}", name);
					valid = false;
					continue;
				};

				let Ok(id_str) = core::str::from_utf8(&name[idx + 1..]) else {
					dbg_warn!(
						"CPU node ID is not a valid UTF-8 string: {:?}",
						&name[idx + 1..]
					);
					valid = false;
					continue;
				};

				let Ok(id) = id_str.parse::<u64>() else {
					dbg_warn!("CPU node ID is not a valid integer: {:?}", id_str);
					valid = false;
					continue;
				};

				cpu_id = id;
			}
			FdtToken::Property { name, value } if name == c"reg" => {
				reg_val = match value.len() {
					1 => value[0].into(),
					2 => {
						value
							.as_ptr()
							.cast::<Be<u16>>()
							.read_unaligned()
							.read()
							.into()
					}
					4 => {
						value
							.as_ptr()
							.cast::<Be<u32>>()
							.read_unaligned()
							.read()
							.into()
					}
					8 => value.as_ptr().cast::<Be<u64>>().read_unaligned().read(),
					_ => {
						dbg_warn!("invalid reg value length: {}", value.len());
						valid = false;
						continue;
					}
				};
			}
			FdtToken::Property { name, value } if name == c"enable-method" => {
				let Ok(value) = CStr::from_bytes_with_nul(value) else {
					dbg_warn!("invalid enable-method value: {value:?}");
					valid = false;
					continue;
				};
				is_psci = value == c"psci";
			}
			FdtToken::Property { name, value } if name == c"device_type" => {
				let Ok(value) = CStr::from_bytes_with_nul(value) else {
					dbg_warn!("invalid device_type value: {value:?}");
					valid = false;
					continue;
				};
				is_psci = value == c"cpu";
			}
			FdtToken::Property { .. } => {}
			FdtToken::Nop | FdtToken::End => unreachable!(),
			FdtToken::EndNode => {
				if !valid {
					continue;
				}

				if !is_psci {
					dbg_warn!("will not boot cpu {cpu_id}: (not enabled via PSCI)");
					continue;
				}

				if !is_cpu {
					dbg_warn!("will not boot cpu {cpu_id}: (not a CPU)");
					continue;
				}

				if reg_val == boot_cpuid.into() {
					dbg!("will not boot cpu {cpu_id} ({reg_val}): (primary core)");
					continue;
				}

				dbg!("booting cpu {cpu_id} ({reg_val})");
				if let Err(err) =
					crate::psci::PsciMethod::Hvc.cpu_on(reg_val, phys, 0xDEAD_BEEF_CABB_A6E5)
				{
					dbg_err!("failed to boot cpu {cpu_id} ({reg_val}): {err:?}");
				}
			}
		}
	}

	crate::asm::halt();
}
