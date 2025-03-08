//! Core local kernel handle for the AArch64 architecture.

use core::cell::UnsafeCell;

use oro_kernel::{arch::Arch, event::Resumption};

/// Core local kernel handle for the AArch64 architecture.
pub struct CoreHandle;

unsafe impl oro_kernel::arch::CoreHandle<crate::Arch> for CoreHandle {
	fn schedule_timer(&self, _ticks: u32) {
		todo!();
	}

	fn cancel_timer(&self) {
		todo!();
	}

	unsafe fn run_context(
		&self,
		_context: Option<&UnsafeCell<<crate::Arch as Arch>::ThreadHandle>>,
		_ticks: Option<u32>,
		_resumption: Option<Resumption>,
	) -> ! {
		todo!();
	}
}
