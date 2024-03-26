//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate includes both a library, with code common to all architectures,
//! and individual, architecture-specific binaries located in `bin/`.
#![no_std]
#![deny(missing_docs)]

use oro_common::{
	boot::{BootInstanceType, KernelBootConfig},
	dbg,
	sync::SpinBarrier,
	Arch,
};

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
pub unsafe fn boot<A: Arch>(
	boot_config: &'static KernelBootConfig,
	boot_instance_type: BootInstanceType,
) -> ! {
	A::disable_interrupts();

	// Bring all cores online
	{
		static BARRIER: SpinBarrier = SpinBarrier::new();

		if boot_instance_type == BootInstanceType::Primary {
			A::init_shared();
			A::init_local();

			// Just to be on the safe side, we initialize
			// the barrier only after we've initialized the core.
			BARRIER.set_total::<A>(boot_config.num_instances);
		} else {
			A::init_local();
		}

		BARRIER.wait();

		if boot_instance_type == BootInstanceType::Primary {
			dbg!(A, "boot", "all cores online");
		}
	}

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
