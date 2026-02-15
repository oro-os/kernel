//! Oro-specific RISC-V64 architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

/// Implements the RISC-V64 architecture.
pub struct Arch;

impl orok_arch_base::Arch for Arch {
	type RawPhysicalAddress = u64;
	type RawVirtualAddress = u64;
}
