use oro_boot_protocol::MemoryMapRequest;

/// The memory map request.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static MMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::with_revision(0);
