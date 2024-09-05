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
pub use self::aarch64::{
	prepare_transfer, transfer, AddressSpace, SupervisorHandle, ELF_CLASS, ELF_ENDIANNESS,
	ELF_MACHINE,
};
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::{
	prepare_transfer, transfer, AddressSpace, SupervisorHandle, ELF_CLASS, ELF_ENDIANNESS,
	ELF_MACHINE,
};
