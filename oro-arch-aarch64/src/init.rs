//! Architecture / core initialization
//! routines and global state definitions.

use core::mem::MaybeUninit;

use oro_kernel::KernelState;
use spin::mutex::fair::FairMutex;

/// The global kernel state. Initialized once during boot
/// and re-used across all cores.
pub static mut KERNEL_STATE: MaybeUninit<KernelState<crate::Arch>> = MaybeUninit::uninit();

/// Initializes the global state of the architecture.
///
/// # Safety
/// Must be called exactly once for the lifetime of the system,
/// only by the boot processor at boot time (_not_ at any
/// subsequent bringup).
pub unsafe fn initialize_primary(pfa: crate::Pfa) {
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

	// SAFETY(qix-): We know what we're doing here.
	#[expect(static_mut_refs)]
	KernelState::init(&mut KERNEL_STATE, FairMutex::new(pfa))
		.expect("failed to create global kernel state");
}

/// Main boot sequence for all cores for each bringup
/// (including boot, including the primary core).
///
/// # Safety
/// Must be called _exactly once_ per core, per core lifetime
/// (i.e. boot, or powerdown/subsequent bringup).
pub unsafe fn boot() -> ! {
	// SAFETY(qix-): THIS MUST ABSOLUTELY BE FIRST.
	#[expect(static_mut_refs)]
	let _kernel = crate::Kernel::initialize_for_core(
		0, // TODO(qix-): pass in the core ID
		KERNEL_STATE.assume_init_ref(),
		(),
	)
	.expect("failed to initialize kernel");

	oro_debug::dbg!("boot");

	crate::asm::halt();
}
