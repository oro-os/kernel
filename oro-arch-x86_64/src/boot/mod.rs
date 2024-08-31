//! Boot routines for the x86_64 architecture.
mod memory;
pub(crate) mod protocol;

use oro_boot_protocol::acpi::AcpiKind;
use oro_debug::dbg;

/// Boots the primary core (boostrap processor) of the system.
///
/// # Safety
/// This function must be called exactly once during boot, and
/// only on the primary core.
///
/// # Panics
/// Panics if any of the boot requests are missing or malformed.
pub unsafe fn boot_primary() -> ! {
	crate::asm::disable_interrupts();
	crate::gdt::install_gdt();
	crate::asm::flush_tlb();

	#[cfg(debug_assertions)]
	oro_debug::init();

	let (_pfa, pat) = memory::prepare_memory();

	crate::reg::Cr0::new()
		.with_monitor_coprocessor()
		.with_emulation()
		.with_alignment_mask()
		.with_paging_enable()
		.inherit()
		.load();

	dbg!("booting primary core");

	// Get the RSDP from the bootloader.
	let AcpiKind::V0(rsdp_response) = protocol::ACPI_REQUEST
		.response()
		.expect("ACPI request was not populated")
	else {
		panic!("ACPI request and response revision number differ");
	};

	let rsdp_phys = rsdp_response.assume_init_ref().rsdp;
	dbg!("ACPI response OK: RSDP at {rsdp_phys:016?}");

	let rsdp = oro_acpi::Rsdp::get(rsdp_phys, pat.clone())
		.expect("RSDP failed to validate; check RSDP pointer");
	dbg!("RSDP revision: {}", rsdp.revision());

	let sdt = rsdp
		.sdt()
		.expect("ACPI tables are missing either the RSDT or XSDT table");

	let madt = sdt
		.find::<oro_acpi::Madt<_>>()
		.expect("MADT table not found in ACPI tables");

	if madt.has_8259() {
		dbg!("8259 PIC detected; disabling it");
		crate::asm::disable_8259();
	}

	for entry in madt.entries() {
		dbg!("MADT entry: {:?}", entry);
	}

	<crate::X86_64 as oro_common::arch::Arch>::halt();
}
