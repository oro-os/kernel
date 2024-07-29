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

use oro_common::{Arch, BootConfig};

/// Core-specific boot information.
///
/// It is up to the architecture-specific implementations
/// to properly initialize this structure and pass it to
/// [`boot()`].
///
/// All general, system-wide configuration should be stored
/// in the boot protocol configuration otherwise.
#[repr(C, align(16))]
pub struct CoreConfig {
	/// The core ID.
	pub core_id:     u64,
	/// The core type.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	pub core_type:   CoreType,
	/// The boot protocol configuration.
	pub boot_config: &'static BootConfig,
	/// The head of the page frame allocator directly
	/// before the transfer.
	pub pfa_head:    u64,
}

/// The core type.
#[derive(PartialEq, Eq, Copy, Clone)]
pub enum CoreType {
	/// The core is the primary core.
	///
	/// # Safety
	/// Exactly one core must be marked as primary.
	Primary,
	/// The core is a secondary core.
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
	#[allow(clippy::missing_docs_in_private_items)]
	macro_rules! wait_for_all_cores {
		() => {{
			static BARRIER: ::oro_common::sync::SpinBarrier =
				::oro_common::sync::SpinBarrier::new();

			if core_config.core_type == CoreType::Primary {
				BARRIER.set_total::<A>(core_config.boot_config.core_count);
			}

			BARRIER.wait();
		}};
	}

	A::disable_interrupts();
	A::after_transfer();

	if core_config.core_type == CoreType::Primary {
		A::init_shared();
	}

	wait_for_all_cores!();

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
