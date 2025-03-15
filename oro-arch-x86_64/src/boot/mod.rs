//! Boot routines for the x86_64 architecture.

pub mod memory;
pub mod primary;
pub mod protocol;
pub mod root_ring;
pub mod secondary;

use core::{cell::UnsafeCell, mem::MaybeUninit};

use oro_debug::{dbg, dbg_warn};
use oro_kernel::GlobalKernelState;

use crate::{
	gdt::{Gdt, SysEntry},
	lapic::Lapic,
	tss::Tss,
};

/// The global kernel state. Initialized once during boot
/// and re-used across all cores.
pub static mut KERNEL_STATE: MaybeUninit<GlobalKernelState<crate::Arch>> = MaybeUninit::uninit();

/// Initializes the core local kernel.
///
/// # Panics
/// Panics if there is an error whilst initializing the kernel
/// state. See [`oro_kernel::Kernel::initialize_for_core`] for
/// more information.
///
/// # Safety
/// Must ONLY be called ONCE for the entire lifetime of the core.
///
/// [`KERNEL_STATE`] must be initialized before calling this function.
/// It's only to be initialized by the primary core at system boot.
/// Secondary cores should assume it's initialized.
pub unsafe fn initialize_core_local(lapic: Lapic) {
	#[expect(static_mut_refs)]
	crate::Kernel::initialize_for_core(
		lapic.id().into(),
		KERNEL_STATE.assume_init_ref(),
		crate::core_local::CoreHandle {
			lapic,
			gdt: UnsafeCell::new(MaybeUninit::uninit()),
			tss: UnsafeCell::new(Tss::default()),
		},
	)
	.expect("failed to initialize kernel");
}

/// Common boot routines for the x86_64 architecture.
///
/// Each processor eventually funnels into this function,
/// which is resposible for finalizing the processor and
/// booting the kernel.
pub fn finalize_boot_and_run() -> ! {
	let kernel = crate::Kernel::get();

	let (tss_offset, gdt) =
		Gdt::<5>::new().with_sys_entry(SysEntry::for_tss(kernel.handle().tss.get()));

	debug_assert_eq!(
		tss_offset,
		crate::gdt::TSS_GDT_OFFSET,
		"TSS offset mismatch"
	);

	let gdt_raw = kernel.handle().gdt.get();
	// SAFETY: This is always valid.
	let gdt_mut = unsafe { &mut *gdt_raw };
	gdt_mut.write(gdt);
	core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
	// SAFETY: We just wrote to the GDT. It's safe to assume it's initialized.
	unsafe {
		gdt_mut.assume_init_ref().install();
	}

	// SAFETY: This is the boot sequence, which is the only place where these functions
	// SAFETY: are called.
	unsafe {
		crate::lapic::initialize_lapic_irqs();
		crate::syscall::install_syscall_handler();
		crate::asm::load_tss(crate::gdt::TSS_GDT_OFFSET);
	}

	if crate::cpuid::CpuidA07C0::get().is_some_and(|c| c.ebx.fsgsbase()) {
		// Allow userspace applications to directly modify FS/GS registers.
		// Further, we disable (for now) the timestamp instruction outside of
		// ring 0.
		// NOTE(qix-): The TSD flag is enabled here tentatively; I need to investigate
		// NOTE(qix-): a bit more the implications of allowing it from userspace applications.
		// SAFETY: We're not modifying any critical flags here that would alter the Rust VM's
		// SAFETY: assumptions about the system state or memory layout.
		unsafe {
			crate::reg::Cr4::load()
				.with_fsgsbase(true)
				.with_tsd(true /* true = cr0 only */)
				.store();
		}
	} else {
		dbg_warn!(
			"CPUID 07:0:EBX.FSGSBASE not supported; not enabling CR4.FSGSBASE (programs may not \
			 work correctly)"
		);
	}

	#[cfg(all(debug_assertions, feature = "dump-cpuid"))]
	{
		use oro_sync::{Lock, Mutex};
		static CPUID_DUMP_LOCK: Mutex<()> = Mutex::new(());

		let lock = CPUID_DUMP_LOCK.lock();

		dbg!(
			"--------------- CPUID :: CPU {} ---------------",
			kernel.id()
		);
		dbg!(
			"CPUID:EAX=01:ECX=00 = {:#?}",
			crate::cpuid::CpuidA01C0::get()
		);
		dbg!(
			"CPUID:EAX=07:ECX=00 = {:#?}",
			crate::cpuid::CpuidA07C0::get()
		);
		dbg!(
			"CPUID:EAX=07:ECX=01 = {:#?}",
			crate::cpuid::CpuidA07C1::get()
		);
		dbg!(
			"CPUID:EAX=07:ECX=02 = {:#?}",
			crate::cpuid::CpuidA07C2::get()
		);
		dbg!(
			"CPUID:EAX=0D:ECX=00 = {:#?}",
			crate::cpuid::CpuidA0DC0::get()
		);
		dbg!(
			"------------- END CPUID :: CPU {} -------------",
			kernel.id()
		);

		drop(lock);
	}

	// Run the kernel, never returning.
	dbg!("booting core {}", kernel.id());

	// SAFETY: NO STACK VALUES MAY BE STORED FOR USAGE BEYOND THIS POINT.
	// SAFETY: THE STACK IS COMPLETELY DESTROYED BY CALLING THIS FUNCTION.
	unsafe {
		kernel.run();
	}
}
