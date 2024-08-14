//! The boot protocol requests that the kernel asks
//! to be populated by the bootloader.
//!
//! For more information about the boot protocol, see
//! [`oro_boot_protocol`].
//!
//! # Architecture Specific Requests
//! **Not all requests the kernel will make are listed here.**
//!
//! The architecture-specific implementations may have
//! additional requests that they make of the bootloader.
//!
//! For a listing of those requests, the architecture-specific
//! crates generally have a `protocol` module that lists
//! the requests.

use oro_boot_protocol::KernelSettingsRequest;

/// The general kernel settings. Applies to all cores.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static KERNEL_SETTINGS: KernelSettingsRequest = KernelSettingsRequest::with_revision(0);
