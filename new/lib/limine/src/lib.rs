#![no_std]

use limine::{BaseRevision, request::HhdmRequest};

/// Provides Limine with a base revision of the protocol
/// that this "kernel" (in Limine terms) expects.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::new();

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
static REQ_STKSZ: limine::request::StackSizeRequest =
	limine::request::StackSizeRequest::new(16 * 1024 * 1024);

/// Requests that Limine performs a Higher Half Direct Map (HHDM)
/// of all physical memory. Provides an offset for the HHDM.
///
/// Note that the boot stage does not rely on an identity map as we
/// will overwrite certain lower-half memory mappings when implementing
/// the stubs (as prescribed by the Oro architectures Limine supports).
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::new();

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
	let _offs: u64 = REQ_HHDM.response().unwrap().offset;

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
	// TODO
	loop {
		core::hint::spin_loop();
	}
}

#[cfg(test)]
fn main() {
	panic!("Don't run this directly.");
}
