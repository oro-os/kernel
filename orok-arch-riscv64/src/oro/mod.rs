//! Oro-specific RISC-V 64-bit architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

mod page_size;
mod unsafe_addr;

/// Implements the RISC-V 64-bit architecture.
#[non_exhaustive]
pub struct Arch;
