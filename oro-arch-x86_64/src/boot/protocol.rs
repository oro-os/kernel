//! Defines the Oro kernel boot requests for the x86_64 architecture.

use oro_boot_protocol::{AcpiRequest, MemoryMapRequest, ModulesRequest};

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
