//! Core local kernel handle for the AArch64 architecture.

/// Core local kernel handle for the AArch64 architecture.
pub struct CoreHandle;

unsafe impl oro_kernel::arch::CoreHandle for CoreHandle {
	fn schedule_timer(&self, _ticks: u32) {
		todo!();
	}

	fn cancel_timer(&self) {
		todo!();
	}
}
