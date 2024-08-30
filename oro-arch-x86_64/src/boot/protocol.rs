//! Defines the Oro kernel boot requests for the x86_64 architecture.

use oro_boot_protocol::{AcpiRequest, MemoryMapRequest};

/// The ACPI root table request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static ACPI_REQUEST: AcpiRequest = AcpiRequest::with_revision(0);

/// The memory map request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static MMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::with_revision(0);
