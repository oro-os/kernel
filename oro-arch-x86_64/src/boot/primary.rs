//! Primary core (also called the bootstrap processor) booting logic.
use oro_acpi::{
	AcpiTable,
	madt::{LocalApicEx as _, MadtEntry},
	sys as acpi_sys,
};
use oro_boot_protocol::acpi::AcpiKind;
use oro_debug::{dbg, dbg_warn};
use oro_mem::{
	mapper::AddressSpace,
	phys::{Phys, PhysAddr},
};

use super::{memory, protocol, secondary};
use crate::mem::address_space::AddressSpaceLayout;

/// Temporary value for the number of stack pages to allocate for secondary cores.
// TODO(qix-): Discover the stack size of the primary core and use that instead.
const SECONDARY_STACK_PAGES: usize = 16;

/// Boots the primary core (boostrap processor) of the system.
///
/// # Safety
/// This function must be called exactly once during boot, and
/// only on the primary core.
///
/// # Panics
/// Panics if any of the boot requests are missing or malformed.
pub unsafe fn boot() -> ! {
	crate::asm::disable_interrupts();
	crate::asm::flush_tlb();
	crate::gdt::GDT.install();
	crate::interrupt::default::install_default_idt();

	#[cfg(debug_assertions)]
	oro_debug::init();

	let memory::PreparedMemory { has_cs89 } = memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(Phys::from_address_unchecked(0).virt());

	dbg!("booting primary core");

	// Get the RSDP from the bootloader.
	let AcpiKind::V0(rsdp_response) = protocol::ACPI_REQUEST
		.response()
		.expect("ACPI request was not populated")
	else {
		panic!("ACPI request and response revision number differ");
	};

	let rsdp_phys = core::ptr::read_volatile(&rsdp_response.assume_init_ref().rsdp);
	dbg!("ACPI response OK: RSDP at {rsdp_phys:016?}");

	let rsdp = oro_acpi::Rsdp::get(rsdp_phys).expect("RSDP failed to validate; check RSDP pointer");
	dbg!("RSDP revision: {}", rsdp.revision());

	let sdt = rsdp
		.sdt()
		.expect("ACPI tables are missing either the RSDT or XSDT table");

	//{
	// 	let mcfg = sdt
	// 		.find::<oro_acpi::Mcfg>()
	// 		.expect("MCFG table not found in ACPI tables");

	// 	for entry in mcfg.entries() {
	// 		dbg!("MCFG entry: {entry:?}", entry = entry);

	// 		let base =
	// 			unsafe { Phys::from_address_unchecked(entry.Address.read()).as_ptr_unchecked() };

	// 		for dev in oro_pci::MmioIterator::new(
	// 			base,
	// 			entry.StartBusNumber.read(),
	// 			entry.EndBusNumber.read(),
	// 		)
	// 		.expect("mis-aligned MCFG PCI(e) base pointer")
	// 		{
	// 			#[allow(unreachable_patterns)]
	// 			match dev.config {
	// 				oro_pci::PciConfig::Type0(config) => {
	// 					dbg!("{dev:?} -> {:#?}", unsafe { config.read_volatile() });
	// 					dbg!("    REGISTERS:");
	// 					for bar in unsafe { (*config).base_registers_iter() } {
	// 						dbg!("        {bar:X?}");
	// 					}
	// 				}
	// 				_ => dbg!("{dev:?} -> ???"),
	// 			}
	// 		}
	// 	}
	//}

	let fadt = sdt
		.find::<oro_acpi::Fadt>()
		.expect("FADT table not found in ACPI tables");
	let fadt = fadt.inner_ref();

	// Enable ACPI if need be.
	if (fadt.Flags.read() & acpi_sys::ACPI_FADT_HW_REDUCED) == 0
		&& !(fadt.SmiCommand.read() == 0
			&& fadt.AcpiEnable.read() == 0
			&& (fadt.Pm1aControlBlock.read() & 1) != 0)
	{
		dbg!("enabling ACPI");
		crate::asm::outb(
			u16::try_from(fadt.SmiCommand.read())
				.expect("ACPI provided an SMI command port that was too large"),
			fadt.AcpiEnable.read(),
		);

		dbg!("enabled ACPI; waiting for it to take effect...");
		let pma1 = u16::try_from(fadt.Pm1aControlBlock.read())
			.expect("ACPI provided a PM1A control block port that was too large");
		while (crate::asm::inw(pma1) & 1) == 0 {
			core::hint::spin_loop();
		}

		dbg!("ACPI enabled");
	} else {
		dbg!("ACPI already enabled");
	}

	let madt = sdt
		.find::<oro_acpi::Madt>()
		.expect("MADT table not found in ACPI tables");

	if madt.has_8259() {
		dbg!("8259 PIC detected; disabling it");
		crate::asm::disable_8259();
	}

	let lapic = crate::lapic::Lapic::new(
		Phys::from_address_unchecked(madt.lapic_phys()).as_mut_ptr_unchecked::<u8>(),
	);
	dbg!("local APIC version: {:?}", lapic.version());
	let lapic_id = lapic.id();
	dbg!("local APIC ID: {lapic_id}");

	{
		#[doc(hidden)]
		#[cfg(not(feature = "force-singlecore"))]
		const MULTICORE_ENABLED: bool = true;
		#[doc(hidden)]
		#[cfg(feature = "force-singlecore")]
		const MULTICORE_ENABLED: bool = false;

		let num_cores = if MULTICORE_ENABLED && has_cs89 {
			dbg!("physical pages 0x8000/0x9000 are valid; attempting to boot secondary cores");

			// Get the current supervisor address space.
			let mapper = AddressSpaceLayout::current_supervisor_space();

			// Boot the secondary cores.
			let mut num_cores = 1; // start at one for the bsp
			for entry in madt.entries().flatten() {
				if let MadtEntry::LocalApic(apic) = entry {
					if apic.can_init() {
						if apic.id() == lapic_id {
							dbg!("cpu {}: not booting (primary core)", apic.id());
						} else {
							dbg!("cpu {}: booting...", apic.id());
							match secondary::boot(&mapper, &lapic, apic.id(), SECONDARY_STACK_PAGES)
							{
								Ok(()) => {
									num_cores += 1;
								}
								Err(err) => {
									dbg_warn!("cpu {} failed to boot: {err:?}", apic.id());
								}
							}
						}
					} else {
						dbg!("cpu {}: not booting (disabled)", apic.id());
					}
				}
			}

			num_cores
		} else {
			if MULTICORE_ENABLED {
				dbg_warn!(
					"physical pages 0x8000/0x9000 are not available; cannot boot secondary cores"
				);
			} else {
				dbg_warn!("multicore disabled; cannot boot secondary cores");
			}

			// Only booting the primary core.
			1
		};

		dbg!("proceeding with {} core(s)", num_cores);
	}

	// The secondaries are now waiting for our signal that the global state has been initialized.
	crate::init::initialize_primary(lapic);

	// Global state has been initialized (along with the primary core's local kernel instance).
	// Signal to the secondaries that they can now proceed with initializing their core-local
	// kernel instances.
	secondary::SECONDARIES_MAY_BOOT.store(true, core::sync::atomic::Ordering::Relaxed);

	// Now boot our own kernel instance.
	crate::init::boot()
}
