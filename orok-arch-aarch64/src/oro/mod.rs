//! Oro-specific AArch64 architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

mod page_size;
mod unsafe_addr;

/// Implements the AArch64 architecture.
#[non_exhaustive]
pub struct Arch;
