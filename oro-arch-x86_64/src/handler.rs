//! Implementation for a [`oro_kernel::scheduler::Handler`] for the x86_64 architecture.

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
		_kernel: &oro_kernel::Kernel<crate::Arch>,
		_thread: &mut oro_kernel::thread::Thread<crate::Arch>,
	) {
		// TODO(qix-): migrate core-local sections.
	}
}
