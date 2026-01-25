#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

/// Implements the AArch64 architecture.
pub struct Arch;

impl orok_arch_base::Arch for Arch {
	type RawPhysicalAddress = u64;
	type RawVirtualAddress = u64;
}
