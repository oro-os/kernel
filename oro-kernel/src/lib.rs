//! Kernel for the [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate includes both a library, with code common to all architectures,
//! and individual, architecture-specific binaries located in `bin/`.
#![no_std]
#![deny(missing_docs)]

use core::{
	mem::MaybeUninit,
	sync::atomic::{AtomicBool, Ordering},
};
use oro_common::{Arch, BootConfig, BootInstanceType};
use spin::Barrier;

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
pub unsafe fn boot<A: Arch>(boot_config: &'static BootConfig) -> ! {
	static BARRIER_INIT: AtomicBool = AtomicBool::new(false);
	static mut BARRIER: MaybeUninit<Barrier> = MaybeUninit::uninit();

	if boot_config.instance_type == BootInstanceType::Primary {
		BARRIER.write(Barrier::new(boot_config.num_instances as usize));
		BARRIER_INIT.store(true, Ordering::Relaxed);

		A::init_shared();
	} else {
		while !BARRIER_INIT.load(Ordering::Relaxed) {
			::core::hint::spin_loop();
		}
	}

	A::init_local();
	BARRIER.assume_init_ref().wait();

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
