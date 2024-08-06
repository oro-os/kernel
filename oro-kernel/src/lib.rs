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

use core::mem::MaybeUninit;
use oro_common::{
	dbg, dbg_err,
	mem::{AddressSpace, FiloPageFrameAllocator, OffsetPhysicalAddressTranslator},
	sync::UnfairSpinlock,
	Arch, BootConfig,
};

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
		(primary $primary:block secondary $secondary:block) => {{
			static BARRIER: ::oro_common::sync::SpinBarrier =
				::oro_common::sync::SpinBarrier::new();
			static AFTER_BARRIER: ::oro_common::sync::SpinBarrier =
				::oro_common::sync::SpinBarrier::new();

			if core_config.core_type == CoreType::Primary {
				$primary
				BARRIER.set_total::<A>(core_config.boot_config.core_count);
				AFTER_BARRIER.set_total::<A>(core_config.boot_config.core_count);
				BARRIER.wait();
			} else {
				BARRIER.wait();
				$secondary
			}

			AFTER_BARRIER.wait();
		}};
		($($t:stmt ;)*) => {{
			// TODO(qix-): Simplify this such that we don't duplicate code.
			wait_for_all_cores! {
				primary {
					$( $t )*
				}
				secondary {
					$( $t )*
				}
			}
		}};
	}

	wait_for_all_cores!();

	// Set up the PFA.
	let translator =
		OffsetPhysicalAddressTranslator::new(core_config.boot_config.linear_map_offset);
	let kernel_addr_space = <A as Arch>::AddressSpace::current_supervisor_space(&translator);

	#[allow(clippy::items_after_statements, clippy::missing_docs_in_private_items)]
	static mut PFA: MaybeUninit<
		UnfairSpinlock<FiloPageFrameAllocator<OffsetPhysicalAddressTranslator>>,
	> = MaybeUninit::uninit();

	if core_config.core_type == CoreType::Primary {
		PFA.write(UnfairSpinlock::new(FiloPageFrameAllocator::with_last_free(
			translator.clone(),
			core_config.pfa_head,
		)));

		A::strong_memory_barrier();
	}

	wait_for_all_cores!();

	// SAFETY(qix-): Since we lockstep initialize the shared PFA, it is safe to
	// SAFETY(qix-): assume that it is initialized here.
	let pfa: &'static _ = &*core::ptr::from_ref(PFA.assume_init_ref());

	wait_for_all_cores! {
		let mut pfa = pfa.lock::<A>();
		A::after_transfer(
			&kernel_addr_space,
			&translator,
			&mut *pfa,
			core_config.core_type == CoreType::Primary,
		);
	}

	if core_config.core_type == CoreType::Primary {
		dbg!(A, "kernel", "kernel transfer ok");
	}

	A::halt()
}

/// Panic handler for the kernel.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic<A: Arch>(info: &::core::panic::PanicInfo) -> ! {
	dbg_err!(A, "kernel", "panic: {:?}", info);
	A::halt()
}
