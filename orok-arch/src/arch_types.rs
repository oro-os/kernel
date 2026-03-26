//! Maps TAIT traits for individual architecture implementations.
//!
//! These mappings are used to build higher level abstractions in `orok-arch`.

#[cfg(target_arch = "aarch64")]
use orok_arch_aarch64::Arch;
#[cfg(target_arch = "riscv64")]
use orok_arch_riscv64::Arch;
#[cfg(target_arch = "x86_64")]
use orok_arch_x86_64::Arch;

#[cfg(not(any(
	target_arch = "x86_64",
	target_arch = "aarch64",
	target_arch = "riscv64"
)))]
compile_error!("unsupported architecture selected for orok-arch");

include!("./arch_types.macro.rs");

impl_type! {
	for Arch {
		use ArchAddressScheme::UnsafePhys as UnsafePhys;
		use ArchAddressScheme::UnsafeVirt as UnsafeVirt;
	}
}

impl_fn! {
	for Arch {
		unsafe fn Halt::halt_once as halt_once();
		unsafe fn Halt::halt as halt() -> !;
	}
}
