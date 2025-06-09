//! Secondary core (application processor) boot routine.

use core::{
	arch::asm,
	ffi::CStr,
	sync::atomic::{AtomicBool, Ordering},
};

use oro_boot_protocol::device_tree::{DeviceTreeDataV0, DeviceTreeKind};
use oro_debug::{dbg, dbg_err, dbg_warn};
use oro_kernel_dtb::{FdtHeader, FdtPathFilter, FdtToken};
use oro_kernel_macro::{asm_buffer, assert};
use oro_kernel_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};
use oro_kernel_type::Be;

use crate::{
	mem::{
		address_space::{AddressSpaceLayout, Ttbr1Handle},
		segment::Segment,
	},
	psci::PsciMethod,
};

/// Brings up all secondary cores.
///
/// Returns the total number of cores in the system
/// (including the primary core).
///
/// # Safety
/// This function is inherently unsafe and must only be called
/// once at kernel boot by the bootstrap processor (primary core).
pub unsafe fn boot_secondaries(stack_pages: usize) -> usize {
	// Get the devicetree blob.
	let DeviceTreeKind::V0(dtb) = super::protocol::DTB_REQUEST
		.response()
		.expect("no DeviceTree blob response was provided")
	else {
		panic!("DeviceTree blob response was provided but was the wrong revision");
	};

	let DeviceTreeDataV0 { base, length } = dtb.assume_init_ref();
	dbg!("got DeviceTree blob of {} bytes", length);

	let dtb = FdtHeader::from(
		Phys::from_address_unchecked(*base).as_ptr().unwrap(),
		Some(*length),
	)
	.expect("dtb is invalid");
	let boot_cpuid = dtb.phys_id();
	dbg!("dtb is valid; primary core id is {boot_cpuid}");

	// Get the PSCI method.
	let mut psci_method: Option<PsciMethod> = None;
	for tkn in dtb.iter().filter_path(&[c"", c"psci"]) {
		match tkn {
			FdtToken::Property { name, value } if name == c"method" => {
				let Ok(value) = CStr::from_bytes_with_nul(value) else {
					dbg_warn!("invalid /psci/method method string: {value:?}");
					continue;
				};

				psci_method = Some(match value {
					v if v == c"hvc" => PsciMethod::Hvc,
					v if v == c"smc" => PsciMethod::Smc,
					unknown => {
						panic!("DTB declared unknown PSCI invocation method: {unknown:?}");
					}
				});
			}
			_ => {}
		}
	}

	let psci = psci_method.expect("no PSCI method was declared in the DTB");

	let version = psci.psci_version().expect("failed to get PSCI version");
	dbg!("PSCI version: {version:?}");

	let mut num_booted = 1;

	let mut is_cpu = true; // Assume it is; `device_type` is deprecated.
	let mut reg_val: u64 = 0;
	let mut is_psci = false;
	let mut cpu_id: u64 = 0;
	let mut valid: bool = false;
	for tkn in dtb.iter().filter_path(&[c"", c"cpus", c"cpu@"]) {
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

				if let Err(err) = boot_secondary(psci, cpu_id, reg_val, stack_pages) {
					dbg_err!("failed to boot cpu {cpu_id} ({reg_val}): {err:?}");
				}

				dbg!("cpu boot {cpu_id} ({reg_val})");
				num_booted += 1;
			}
		}
	}

	num_booted
}

/// Error type for secondary core booting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(variant_size_differences)]
enum SecondaryBootError {
	/// The system ran out of memory when allocating space for the secondary core.
	OutOfMemory,
	/// A PSCI error was returned
	PsciError(crate::psci::Error),
	/// There was a failure when mapping memory into the secondary core.
	MapError(MapError),
}

/// The secondary boot core initialization block.
///
/// The physical address of this structure is passed
/// to the secondary core via the `x0` register.
#[derive(Debug)]
#[repr(C, align(4096))]
struct BootInitBlock {
	/// The core's unique ID.
	///
	/// This is a logical ID that has no bearing on the
	/// core's MPIDR value or anything related to PSCI.
	core_id:        u64,
	/// The physical address of the `TTBR0_EL1` register for the core.
	ttbr0_el1:      u64,
	/// The physical address of the `TTBR1_EL1` register for the core.
	ttbr1_el1:      u64,
	/// The `TCR_EL1` register for the core.
	tcr_el1:        u64,
	/// The stack pointer for the core.
	stack_pointer:  u64,
	/// The MAIR register value for the core.
	mair:           u64,
	/// Where to jump to after initialization.
	///
	/// This is the _virtual_ address after the `TTBR1_EL1`
	/// register has been set.
	///
	/// Core stub must forward the `x0` register
	/// to this address as-is (thus must not
	/// clobber x0).
	entry_point:    u64,
	/// The linear offset to use for the PAT.
	linear_offset:  u64,
	/// Primary flag. Performs lock-step green-lighting
	/// of secondary core execution.
	primary_flag:   AtomicBool,
	/// Secondary flag. Indicates that the secondary core
	/// has been booted and is running.
	secondary_flag: AtomicBool,
}

asm_buffer! {
	/// The secondary boot core initialization stub.
	///
	/// Upon entry, `x0` is the physical address of the
	/// `BootInitBlock` structure that parameterizes
	/// the core.
	///
	/// This physical address must be forwarded in
	/// `x0` to the `entry_point` field of the `BootInitBlock`
	/// when branching to the kernel.
	///
	/// Expects that the page these are written to is
	/// direct-mapped. The `BootInitBlock` does not
	/// need to be.
	static SECONDARY_BOOT_STUB: AsmBuffer = {
		{
			// Make sure the MMU is disabled (it should be).
			"mrs x9, sctlr_el1",
			"bic x9, x9, #1",
			"msr sctlr_el1, x9",

			// Set up the MAIR register (0x28)
			"ldr x9, [x0, #0x28]",
			"msr mair_el1, x9",

			// Set up the TCR_EL1 registers (0x18)
			"ldr x9, [x0, #0x18]",
			"msr tcr_el1, x9",

			// Set up the TTBR0_EL1/TTBR1_EL1 registers (0x8, 0x10)
			"ldr x9, [x0, #0x8]",
			"msr ttbr0_el1, x9",
			"ldr x9, [x0, #0x10]",
			"msr ttbr1_el1, x9",

			// Load the entry point we'll jump to (0x30)
			"ldr x10, [x0, #0x30]",

			// Set the stack pointer (0x20)
			"ldr x9, [x0, #0x20]",
			"mov sp, x9",

			// Add the linear offset to the init block base address.
			//
			// IMPORTANT: BootInitBlock is no longer available
			// IMPORTANT: after this point.
			"ldr x9, [x0, #0x38]",
			"add x0, x0, x9",

			// Invalidate TLBs
			"tlbi vmalle1is",
			"ic iallu",
			"dc isw, xzr",
			"dsb nsh",
			"isb",

			// Re-enable the MMU
			"mrs x9, sctlr_el1",
			"orr x9, x9, #1",
			"msr sctlr_el1, x9",

			// Invalidate TLBs
			"tlbi vmalle1is",
			"ic iallu",
			"dc isw, xzr",
			"dsb nsh",
			"isb",

			// Jump to the kernel
			"br x10",
		}
	};
}

/// Attempts to boot a single secondary core.
unsafe fn boot_secondary(
	psci: PsciMethod,
	cpu_id: u64,
	reg_val: u64,
	stack_pages: usize,
) -> Result<(), SecondaryBootError> {
	// Get the primary handle.
	let primary_mapper = AddressSpaceLayout::current_supervisor_space();

	// Create a new supervisor address space based on the current address space.
	let mapper = AddressSpaceLayout::duplicate_supervisor_space_shallow(&primary_mapper)
		.ok_or(SecondaryBootError::OutOfMemory)?;

	// Also create an empty mapper for the TTBR0_EL1 space.
	let lower_mapper =
		AddressSpaceLayout::new_supervisor_space_ttbr0().ok_or(SecondaryBootError::OutOfMemory)?;

	// Allocate the boot stubs (maximum 4096 bytes).
	let boot_phys = GlobalPfa
		.allocate()
		.ok_or(SecondaryBootError::OutOfMemory)?;
	let boot_virt = Phys::from_address_unchecked(boot_phys).as_mut_ptr_unchecked::<[u8; 4096]>();
	(&mut *boot_virt)[..SECONDARY_BOOT_STUB.len()].copy_from_slice(&SECONDARY_BOOT_STUB);

	// Direct map the boot stubs into the lower page table.
	AddressSpaceLayout::stubs()
		.map(&lower_mapper, boot_phys as usize, boot_phys)
		.map_err(SecondaryBootError::MapError)?;

	// Forget the stack in the upper address space.
	AddressSpaceLayout::kernel_stack().unmap_without_reclaim(&mapper);

	// Allocate a new stack for it...
	let stack_segment = AddressSpaceLayout::kernel_stack();
	let stack_end = <&Segment as AddressSegment<Ttbr1Handle>>::range(&stack_segment).1 & !0xFFF;

	for stack_virt in (stack_end - stack_pages * 4096..stack_end).step_by(4096) {
		let page = GlobalPfa
			.allocate()
			.ok_or(SecondaryBootError::OutOfMemory)?;
		stack_segment
			.map(&mapper, stack_virt, page)
			.map_err(SecondaryBootError::MapError)?;
	}

	// Get a copy of relevant registers from the primary core.
	let mair_val: u64 = crate::mair::MairEntry::build_mair().into();
	let tcr: u64 = crate::reg::tcr_el1::TcrEl1::load().into();

	// Write the boot init block.
	assert::fits::<BootInitBlock, 4096>();
	let init_block_phys = GlobalPfa
		.allocate()
		.ok_or(SecondaryBootError::OutOfMemory)?;
	let init_block_ptr =
		Phys::from_address_unchecked(init_block_phys).as_mut_ptr_unchecked::<BootInitBlock>();
	debug_assert!(init_block_ptr.is_aligned());
	init_block_ptr.write(BootInitBlock {
		core_id:        cpu_id,
		ttbr0_el1:      lower_mapper.base_phys.address_u64(),
		ttbr1_el1:      mapper.base_phys.address_u64(),
		tcr_el1:        tcr,
		stack_pointer:  stack_end as u64,
		mair:           mair_val,
		entry_point:    boot_secondary_entry as *const u8 as u64,
		linear_offset:  Phys::from_address_unchecked(0).virt() as u64,
		primary_flag:   AtomicBool::new(false),
		secondary_flag: AtomicBool::new(false),
	});

	psci.cpu_on(reg_val, boot_phys, init_block_phys)
		.map_err(SecondaryBootError::PsciError)?;

	// Tell the secondary core we're ready.
	let init_block = &*init_block_ptr;
	init_block.primary_flag.store(true, Ordering::Release);

	// Wait for the secondary core to boot.
	while !init_block.secondary_flag.load(Ordering::Acquire) {
		core::hint::spin_loop();
	}

	// XXX(qix-): Do we need to unmap something here?

	Ok(())
}

/// The Rust entry point for the secondary cores after they've been booted
/// and initialized by the assembly stub.
#[inline(never)]
#[unsafe(no_mangle)]
unsafe extern "C" fn boot_secondary_entry() {
	let boot_block_virt: u64;
	asm!("", out("x0") boot_block_virt);

	crate::asm::disable_interrupts();

	let boot_block = &*(boot_block_virt as *const BootInitBlock);

	// Wait for the primary core to give the green light..
	while !boot_block.primary_flag.load(Ordering::Acquire) {
		core::hint::spin_loop();
	}

	// Signal back that we're headed off.
	//
	// After this point, our TTBR0_EL1 address
	// space is gone, including the boot_block_virt.
	boot_block.secondary_flag.store(true, Ordering::Release);
	let _ = boot_block;
	let _ = boot_block_virt;
	asm!("msr ttbr0_el1, xzr");

	// The logger should already be initialized
	// by the primary core.
	dbg!("secondary core {} booted", boot_block.core_id);

	crate::init::boot();
}
