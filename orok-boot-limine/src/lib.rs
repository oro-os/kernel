#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]

#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{BaseRevision, request::HhdmRequest};

/// Provides Limine with a base revision of the protocol
/// that this "kernel" (in Limine terms) expects.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(2);

/// In debug builds, stack size is very quickly exhausted. At time
/// of writing, Limine allocates 64KiB of stack space per core, but
/// this is not enough for debug builds.
///
/// Further, since there are no stack fences or automatic stack growing
/// implemented in this stage, we must ensure there's enough stack space
/// available for the debug build to avoid a stack overflow and subsequent
/// corruption of kernel memory.
///
/// Thus, we expand the stack size here, fairly substantially.
#[cfg(debug_assertions)]
#[used]
static REQ_STKSZ: StackSizeRequest = StackSizeRequest::with_revision(0).with_size(16 * 1024 * 1024);

/// Requests that Limine performs a Higher Half Direct Map (HHDM)
/// of all physical memory. Provides an offset for the HHDM.
///
/// Note that the boot stage does not rely on an identity map as we
/// will overwrite certain lower-half memory mappings when implementing
/// the stubs (as prescribed by the Oro architectures Limine supports).
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::with_revision(0);

#[orok_test::effect(write_reg = cr0)]
#[inline(never)]
fn sets_cr0() {
	// We don't set CR0 here, just to test if the effects system works.
	// If we _did_, QEMU would pick it up and would emit that event,
	// which would cause the constraint check to succeed. I want it to
	// fail for now, just to test.
	::core::hint::black_box(());
}

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// # Panics
/// Panics if required responses aren't populated by Limine
pub unsafe fn init() -> ! {
	let offs = REQ_HHDM.get_response().unwrap().offset();
	// SAFETY: We've ensured this is valid before any MMIO writes occur.
	unsafe {
		orok_test::set_vmm_base(offs);
	}
	// Must be first.
	orok_test::oro_has_started_execution!();

	sets_cr0();

	panic!();
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
	loop {
		// SAFETY: Inline assembly is required to halt the CPU.
		unsafe {
			#[cfg(target_arch = "aarch64")]
			core::arch::asm!("wfi");
			#[cfg(target_arch = "x86_64")]
			core::arch::asm!("cli; hlt");
			#[cfg(target_arch = "riscv64")]
			core::arch::asm!("wfi");
		}
	}
}
