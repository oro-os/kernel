//! Defines the Oro kernel boot requests for the x86_64 architecture.

use oro_boot_protocol::{AcpiRequest, KernelSettingsRequest};

/// The general kernel settings. Applies to all cores.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static KERNEL_SETTINGS: KernelSettingsRequest = KernelSettingsRequest::with_revision(0);

/// The ACPI root table request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static ACPI_REQUEST: AcpiRequest = AcpiRequest::with_revision(0);
