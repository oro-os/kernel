//! Architecture / core initialization
//! routines and global state definitions.

use core::mem::MaybeUninit;
use oro_kernel::{Kernel, KernelState};
use oro_mem::{pfa::filo::FiloPageFrameAllocator, translate::OffsetTranslator};
use oro_sync::spinlock::unfair_critical::UnfairCriticalSpinlock;

/// Type alias for the PFA (page frame allocator) implementation used
/// by the architecture.
pub type Pfa = FiloPageFrameAllocator<OffsetTranslator>;

/// The global kernel state. Initialized once during boot
/// and re-used across all cores.
pub static mut KERNEL_STATE: MaybeUninit<
	KernelState<
		Pfa,
		OffsetTranslator,
		crate::mem::address_space::AddressSpaceLayout,
		crate::sync::InterruptController,
	>,
> = MaybeUninit::uninit();

/// Initializes the global state of the architecture.
///
/// # Safety
/// Must be called exactly once for the lifetime of the system,
/// only by the boot processor at boot time (_not_ at any
/// subsequent bringup).
pub unsafe fn initialize_primary(pat: OffsetTranslator, pfa: Pfa) {
	#[cfg(debug_assertions)]
	{
		use core::sync::atomic::{AtomicBool, Ordering};

		#[doc(hidden)]
		static HAS_INITIALIZED: AtomicBool = AtomicBool::new(false);

		if HAS_INITIALIZED
			.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
			.is_err()
		{
			panic!("init() called more than once");
		}
	}

	KERNEL_STATE.write(
		KernelState::new(pat, UnfairCriticalSpinlock::new(pfa))
			.expect("failed to create global kernel state"),
	);
}

/// Main boot sequence for all cores for each bringup
/// (including boot, including the primary core).
///
/// # Safety
/// Must be called _exactly once_ per core, per core lifetime
/// (i.e. boot, or powerdown/subsequent bringup).
pub unsafe fn boot() -> ! {
	let _kernel = Kernel::new(KERNEL_STATE.assume_init_ref());

	oro_debug::dbg!("boot");

	crate::asm::halt();
}
