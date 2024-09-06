//! Kernel boot protocol requests for the AArch64 architecture.

use oro_boot_protocol::{DeviceTreeRequest, MemoryMapRequest};

/// The memory map request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static MMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::with_revision(0);

/// The DeviceTree blob request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static DTB_REQUEST: DeviceTreeRequest = DeviceTreeRequest::with_revision(0);
