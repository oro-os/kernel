//! Implementation of [`oro_kernel::scheduler::Handler`] for the x86_64 architecture.

use oro_mem::mapper::AddressSpace;

use crate::mem::address_space::AddressSpaceLayout;

/// x86_64 [`oro_kernel::scheduler::Handler`] implementation
/// for the Oro kernel scheduler.
pub struct Handler {
	/// Static reference to this core's kernel.
	kernel: &'static crate::Kernel,
}

impl Handler {
	/// Creates a new handler instance, caching
	/// a reference to the kernel.
	#[must_use]
	pub fn new() -> Self {
		Self {
			kernel: crate::Kernel::get(),
		}
	}

	/// Gets the kernel reference.
	pub(crate) fn kernel(&self) -> &'static crate::Kernel {
		self.kernel
	}
}

impl oro_kernel::scheduler::Handler<crate::Arch> for Handler {
	fn schedule_timer(&self, ticks: u32) {
		self.kernel.core().lapic.set_timer_initial_count(ticks);
	}

	fn cancel_timer(&self) {
		self.kernel.core().lapic.cancel_timer();
	}

	fn migrate_thread(
		kernel: &oro_kernel::Kernel<crate::Arch>,
		thread: &mut oro_kernel::thread::Thread<crate::Arch>,
	) {
		let mapper = kernel.mapper();
		let pat = kernel.state().pat();

		// Re-map the stack and core-local mappings.
		// SAFETY(qix-): We don't need to reclaim anything so overwriting the mappings
		// SAFETY(qix-): is safe.
		unsafe {
			let thread_mapper = thread.mapper();
			AddressSpaceLayout::kernel_stack().mirror_into(thread_mapper, mapper, pat);
			AddressSpaceLayout::kernel_core_local().mirror_into(thread_mapper, mapper, pat);
		}
	}
}
