//! Boot protocol types and serializer implementation.
//! Provides boot-time information to the kernel from the boot
//! stage configuration.
//!
//! Note that no core-specific information is provided here, as
//! that is handled by passing information to the kernel via
//! architecture-specific transfer stubs.
#![allow(rustdoc::private_intra_doc_links)]

use crate::ser2mem::Ser2Mem;

/// Root structure of the kernel boot protocol, sent to the kernel via
/// serialization to memory.
///
/// # Safety
/// Do not instantiate this structure yourself; you probably won't
/// be able to anyway, but it's meant to be instantiated via its
/// [`crate::ser2mem::Proxy`] implementation (auto-generated via
/// the `#[derive(Ser2Mem)]` macro) and then serialized to memory.
#[derive(Ser2Mem, Debug)]
#[repr(C)]
pub struct BootConfig {
	/// The total number of cores being booted.
	pub core_count:        u64,
	/// The virtual offset of the linear map of physical memory.
	pub linear_map_offset: usize,
}
