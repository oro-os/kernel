//! Architecture / core initialization
//! routines and global state definitions.

use crate::{handler::Handler, lapic::Lapic};
use core::mem::MaybeUninit;
use oro_debug::dbg;
use oro_kernel::KernelState;
use oro_mem::translate::OffsetTranslator;
use oro_sync::spinlock::unfair_critical::UnfairCriticalSpinlock;

/// The global kernel state. Initialized once during boot
/// and re-used across all cores.
pub static mut KERNEL_STATE: MaybeUninit<KernelState<crate::Arch>> = MaybeUninit::uninit();

/// Initializes the global state of the architecture.
///
/// # Safety
/// Must be called exactly once for the lifetime of the system,
/// only by the boot processor at boot time (_not_ at any
/// subsequent bringup).
pub unsafe fn initialize_primary(pat: OffsetTranslator, pfa: crate::Pfa) {
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
	KernelState::init(&mut KERNEL_STATE, pat, UnfairCriticalSpinlock::new(pfa))
		.expect("failed to create global kernel state");

	// XXX TODO(qix-): list out the modules the bootloader sent
	if let Some(oro_boot_protocol::modules::ModulesKind::V0(modules)) =
		crate::boot::protocol::MODULES_REQUEST.response()
	{
		let modules = modules.assume_init_ref();
		dbg!("got modules next: {:016x}", modules.next);
	}
}

/// Main boot sequence for all cores for each bringup
/// (including boot, including the primary core).
///
/// # Safety
/// Must be called _exactly once_ per core, per core lifetime
/// (i.e. boot, or powerdown/subsequent bringup).
pub unsafe fn boot(lapic: Lapic) -> ! {
	// SAFETY(qix-): THIS MUST ABSOLUTELY BE FIRST.
	let _ = crate::Kernel::initialize_for_core(
		KERNEL_STATE.assume_init_ref(),
		crate::CoreState { lapic },
	)
	.expect("failed to initialize kernel");

	crate::interrupt::install_idt();

	dbg!("boot");

	let handler = Handler::new();
	loop {
		if let Some(_user_ctx) = handler.kernel().scheduler().event_idle(&handler) {
			todo!();
		} else {
			// Nothing to do. Wait for an interrupt.
			// Scheduler will have asked us to set a timer
			// if it wants to be woken up.
			crate::asm::halt_once();
		}
	}
}
