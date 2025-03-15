//! Defines the Oro kernel boot requests for the x86_64 architecture.

use oro_boot_protocol::{AcpiRequest, MemoryMapRequest, ModulesRequest, acpi::AcpiKind};

/// The ACPI root table request.
///
/// Required.
#[used]
#[unsafe(link_section = ".oro_boot")]
pub static ACPI_REQUEST: AcpiRequest = AcpiRequest::with_revision(0);

/// The memory map request.
///
/// Required.
#[used]
#[unsafe(link_section = ".oro_boot")]
pub static MMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::with_revision(0);

/// The modules request.
///
/// Optional (but not very useful if not provided).
/// If omitted, treated as though `.next` is `0`.
#[used]
#[unsafe(link_section = ".oro_boot")]
pub static MODULES_REQUEST: ModulesRequest = ModulesRequest::with_revision(0);

/// Uses the ACPI request to try to find an ACPI table by name.
///
/// Returns `None` if no ACPI protocol was supplied, or if no table
/// was found.
///
/// # Panics
/// Panics if the ACPI request and response revision number differ.
#[must_use]
pub fn find_acpi_table<T: oro_acpi::AcpiTable>() -> Option<T> {
	// Get the RSDP from the bootloader.
	let AcpiKind::V0(rsdp_response) = ACPI_REQUEST.response()? else {
		panic!("ACPI request and response revision number differ");
	};
	// SAFETY: We have to assume this is safe; otherwise it's a bug in the OEM's ACPI tables.
	let rsdp_phys = unsafe { core::ptr::read_volatile(&rsdp_response.assume_init_ref().rsdp) };
	// SAFETY: We have to assume this is safe; otherwise it's either a bug in the OEM's ACPI
	// SAFETY: tables or a bug in the bootloader.
	let rsdp = unsafe { oro_acpi::Rsdp::get(rsdp_phys)? };
	let sdt = rsdp.sdt()?;
	sdt.find::<T>()
}
