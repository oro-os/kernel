//! Abstracts over the target platform, exposing a trait-bound
//! type alias for interacting with certain architecture facilities.
//!
//! One of the few places in the codebase that uses arch-conditional
//! compilation.

// NOTE(qix-): We could probably figure out a way to use traits to switch
// NOTE(qix-): on archs instead of the current method, but this is the
// NOTE(qix-): simplest way to do it for now.

// NOTE(qix-): Documenting this with `#[cfg(any(doc, ...))]` doesn't work
// NOTE(qix-): since 1) --cfg doc isn't passed to dependencies, and
// NOTE(qix-): 2) we're exporting the same symbol names based on the arch,
// NOTE(qix-): which causes naming conflicts when generating multi-arch docs.

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
pub use self::aarch64::{
	AddressSpace, ELF_CLASS, ELF_ENDIANNESS, ELF_MACHINE, SupervisorHandle, prepare_transfer,
	transfer,
};
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::{
	AddressSpace, ELF_CLASS, ELF_ENDIANNESS, ELF_MACHINE, SupervisorHandle, prepare_transfer,
	transfer,
};
