//! Oro-specific RISC-V 64-bit architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

mod page_size;
mod unsafe_addr;

/// Implements the RISC-V 64-bit architecture.
pub struct Arch;

impl orok_arch_base::Arch for Arch {
	type PageSize = page_size::PageSize;
	type UnsafePhys = unsafe_addr::UnsafePhys;
	type UnsafeVirt = unsafe_addr::UnsafeVirt;
}
