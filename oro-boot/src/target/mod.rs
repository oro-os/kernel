//! Abstracts over the target platform, exposing a trait-bound
//! type alias for interacting with certain architecture facilities.
//!
//! One of the few places in the codebase that uses arch-conditional
//! compilation.

// NOTE(qix-): We could probably figure out a way to use traits to switch
// NOTE(qix-): on archs instead of the current method, but this is the
// NOTE(qix-): simplest way to do it for now.

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
pub use self::aarch64::{prepare_transfer, transfer, AddressSpace, SupervisorHandle};
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::{prepare_transfer, transfer, AddressSpace, SupervisorHandle};

/// The target architecture for the Oro kernel.
pub type TargetArch = impl oro_common::arch::Arch;

#[doc(hidden)]
#[allow(dead_code)]
#[cfg(target_arch = "x86_64")]
fn _x86_64() -> TargetArch {
	::oro_arch_x86_64::X86_64
}

#[doc(hidden)]
#[allow(dead_code)]
#[cfg(target_arch = "aarch64")]
fn _aarch64() -> TargetArch {
	::oro_arch_aarch64::Aarch64
}
