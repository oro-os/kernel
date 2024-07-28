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

use oro_common::Arch;

/// Core-specific boot information.
///
/// It is up to the architecture-specific implementations
/// to properly initialize this structure and pass it to
/// [`boot()`].
///
/// All general, system-wide configuration should be stored
/// in the boot protocol configuration otherwise.
#[derive(Default)]
#[repr(C, align(16))]
pub struct CoreConfig {
	/// The core ID.
	pub core_id:   u64,
	/// The core type.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	pub core_type: CoreType,
}

/// The core type.
#[derive(Default, PartialEq, Eq, Copy, Clone)]
pub enum CoreType {
	/// The core is the primary core.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	Primary,
	/// The core is a secondary core.
	#[default]
	Secondary,
}

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
///
/// The `core_config` parameter must be properly initialized.
/// Specifically, all safety requirements must be met, such as
/// marking exactly one core as primary.
pub unsafe fn boot<A: Arch>(core_config: &CoreConfig) -> ! {
	A::disable_interrupts();
	A::after_transfer();

	if core_config.core_type == CoreType::Primary {
		A::init_shared();
	}

	// TODO(qix-): barrier. But we need the core count from the boot protocol info first.

	A::init_local();

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
