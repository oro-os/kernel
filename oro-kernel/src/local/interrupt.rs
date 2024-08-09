//! Implements the interrupt handlers for the kernel.

use crate::local::state::core_state;
use oro_common::interrupt::InterruptHandler;

/// The main interrupt handler for the kernel.
pub struct KernelInterruptHandler;

unsafe impl InterruptHandler for KernelInterruptHandler {
	// 100hz, or 10ms.
	// Pretty universally accepted tick rate for preemption.
	const TARGET_TICK_RATE_HZ: u64 = 100;

	unsafe fn handle_tick() {
		let core_state = core_state();
		core_state.ticked.write(1);
	}
}
