//! Primary core (also called the bootstrap processor) booting logic.
use oro_acpi::{
	AcpiTable,
	madt::{LocalApicEx as _, MadtEntry},
	sys as acpi_sys,
};
use oro_debug::{dbg, dbg_warn};
use oro_kernel::GlobalKernelState;
use oro_mem::{
	alloc::sync::Arc,
	mapper::AddressSpace,
	phys::{Phys, PhysAddr},
};

use super::{memory, secondary};
use crate::{mem::address_space::AddressSpaceLayout, time::GetInstant};

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
	crate::interrupt::install_default();

	#[cfg(debug_assertions)]
	oro_debug::init();

	let memory::PreparedMemory { has_cs89 } = memory::prepare_memory();

	// We now have a valid physical map; let's re-init
	// any MMIO loggers with that offset.
	#[cfg(debug_assertions)]
	oro_debug::init_with_offset(Phys::from_address_unchecked(0).virt());

	dbg!("booting primary core");

	let fadt = crate::boot::protocol::find_acpi_table::<oro_acpi::Fadt>()
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

	let madt = crate::boot::protocol::find_acpi_table::<oro_acpi::Madt>()
		.expect("MADT table not found in ACPI tables");

	if madt.has_8259() {
		dbg!("8259 PIC detected; disabling it");
		// TODO(qix-): Detect if the IMCR is enabled before passing `true`.
		// TODO(qix-): https://github.com/oro-os/development-notes/blob/master/Development%20Notes/x86/Scheduler%20Refactor%20(Mar%20'25).md#7-march-2025-apic-troubles
		unsafe {
			crate::asm::disable_8259(true);
		}
	}

	let lapic = crate::lapic::Lapic::new(
		Phys::from_address_unchecked(madt.lapic_phys()).as_mut_ptr_unchecked::<u8>(),
	);
	dbg!("local APIC version: {:?}", lapic.version());
	let lapic_id = lapic.id();
	dbg!("local APIC ID: {lapic_id}");

	let timekeeper: Arc<dyn GetInstant> = crate::hpet::initialize();

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
							match secondary::boot(
								&mapper,
								&lapic,
								apic.id(),
								SECONDARY_STACK_PAGES,
								timekeeper.clone(),
							) {
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

	// SAFETY(qix-): This is the only place we take a mutable reference to it.
	#[expect(static_mut_refs)]
	GlobalKernelState::init(&mut super::KERNEL_STATE)
		.expect("failed to create global kernel state");

	super::initialize_core_local(lapic, timekeeper);
	// SAFETY: This is the only place where the root ring is being initialized.
	unsafe {
		super::root_ring::initialize_root_ring();
	}

	// Global state has been initialized (along with the primary core's local kernel instance).
	// Signal to the secondaries that they can now proceed with initializing their core-local
	// kernel instances.
	secondary::SECONDARIES_MAY_BOOT.store(true, core::sync::atomic::Ordering::Relaxed);

	// Now boot the kernel.
	super::finalize_boot_and_run();
}
