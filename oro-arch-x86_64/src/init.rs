//! Architecture / core initialization
//! routines and global state definitions.

use crate::{
	gdt::{Gdt, SysEntry},
	handler::Handler,
	lapic::Lapic,
	tss::Tss,
};
use core::{cell::UnsafeCell, mem::MaybeUninit};
use oro_debug::{dbg, dbg_warn};
use oro_kernel::KernelState;
use oro_mem::translate::{OffsetTranslator, Translator};
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
#[expect(clippy::needless_pass_by_value)]
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
	KernelState::init(
		&mut KERNEL_STATE,
		pat.clone(),
		UnfairCriticalSpinlock::new(pfa),
	)
	.expect("failed to create global kernel state");

	// TODO(qix-): Not sure that I like that this is ELF-aware. This may get
	// TODO(qix-): refactored at some point.
	if let Some(oro_boot_protocol::modules::ModulesKind::V0(modules)) =
		crate::boot::protocol::MODULES_REQUEST.response()
	{
		let modules = modules.assume_init_ref();
		let mut next = modules.next;

		while next != 0 {
			let module = &*pat.translate::<oro_boot_protocol::Module>(next);
			next = module.next;

			let id = oro_id::AnyId::from_high_low(module.id_high, module.id_low);

			let Ok(id) = oro_id::Id::<{ oro_id::IdType::Module }>::try_from(id) else {
				dbg_warn!(
					"skipping module; not a valid module ID: {:?}",
					id.as_bytes()
				);
				continue;
			};

			if id.is_internal() {
				dbg_warn!("skipping module; internal module ID: {:?}", id.as_bytes());
				continue;
			}

			dbg!(
				"loading module: {id} @ {:016X} ({})",
				module.base,
				module.length
			);
		}
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
	let kernel = crate::Kernel::initialize_for_core(
		KERNEL_STATE.assume_init_ref(),
		crate::CoreState {
			lapic,
			gdt: UnsafeCell::new(MaybeUninit::uninit()),
			tss: UnsafeCell::new(Tss::default()),
		},
	)
	.expect("failed to initialize kernel");

	let (tss_offset, gdt) =
		Gdt::<5>::new().with_sys_entry(SysEntry::for_tss(kernel.core().tss.get()));

	assert_eq!(tss_offset, crate::TSS_GDT_OFFSET, "TSS offset mismatch");

	{
		let gdt_raw = kernel.core().gdt.get();
		let gdt_mut = &mut *gdt_raw;
		gdt_mut.write(gdt);
		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
		gdt_mut.assume_init_ref().install();
	}

	crate::interrupt::install_idt();
	crate::asm::load_tss(crate::TSS_GDT_OFFSET);

	dbg!("boot");

	let handler = Handler::new();
	loop {
		crate::asm::disable_interrupts();
		if let Some(_user_ctx) = handler.kernel().scheduler().event_idle(&handler) {
			crate::asm::enable_interrupts();
			todo!();
		} else {
			// Nothing to do. Wait for an interrupt.
			// Scheduler will have asked us to set a timer
			// if it wants to be woken up.
			crate::asm::enable_interrupts();
			crate::asm::halt_once();
		}
	}
}
