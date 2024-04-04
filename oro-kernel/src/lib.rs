//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate includes both a library, with code common to all architectures,
//! and individual, architecture-specific binaries located in `bin/`.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]

use oro_common::{dbg, Arch};

/// Runs the kernel.
///
/// This is the main entry point for the kernel.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// Further, all architecture-specific setup MUST have completed
/// on ALL CORES before calling this function.
pub unsafe fn boot<A: Arch>() -> ! {
	A::disable_interrupts();
	dbg!(A, "kernel", "hello");
	A::halt()
}

/// Panic handler for the kernel.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic<A: Arch>(_info: &::core::panic::PanicInfo) -> ! {
	A::halt()
}
