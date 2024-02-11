//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]
#![deny(missing_docs)]

use oro_common::{dbg, dbg_err, dbg_warn, Arch};

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn init<A: Arch>() -> ! {
	// We know that there is only one CPU being used
	// in the bootloader stage.
	A::init_shared();
	A::init_local();

	dbg!(A, "limine", "general");
	dbg_warn!(A, "limine", "warning");
	dbg_err!(A, "limine", "error");

	A::halt() // TODO(qix-): Temporary.
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic<A: Arch>(_info: &::core::panic::PanicInfo) -> ! {
	A::halt()
}
