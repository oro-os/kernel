#![cfg_attr(
	not(doc),
	expect(missing_docs, reason = "docs are enabled only under `doc` cfg")
)]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg))]
#![cfg_attr(not(test), no_main)]

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

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// # Panics
/// Panics if required responses aren't populated by Limine.
#[expect(
	clippy::unwrap_used,
	clippy::panic,
	reason = "kernel has no choice but to panic in this function"
)]
pub unsafe fn init() -> ! {
	let offs = REQ_HHDM.get_response().unwrap().offset();

	// SAFETY: We've ensured this is valid before any MMIO writes occur.
	unsafe {
		orok_test::set_vmm_base(offs);
	}

	// Must be first test effect that is emitted.
	orok_test::oro_has_started_execution!();

	panic!();
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[panic_handler]
unsafe fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
	// SAFETY: We're aware we're about to halt.
	unsafe {
		orok_arch::halt();
	}
}

/// Main entry point for the Limine bootloader stage
/// for the Oro kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It is called
/// by the Limine bootloader.
#[inline(never)]
#[cfg(not(test))]
#[cold]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
	// SAFETY: This is the architecture-specific entry function, the
	// SAFETY: only allowed place to call this function.
	unsafe { init() }
}

#[cfg(test)]
fn main() {
	panic!("Don't run this directly.");
}
