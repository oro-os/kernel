use core::num::NonZero;

use ::oro::{id::kernel::iface::KERNEL_THREAD_V0, syscall};

use crate::thread::Thread;

/// Gets a handle to the thread that invokes it.
///
/// # Oro-specific
/// This function **panics** in the rare case the thread handle cannot be retrieved.
/// This is a temporary measure until the kernel implements TLS.
#[expect(clippy::missing_panics_doc)]
#[must_use]
pub fn current() -> Thread {
	// NOTE(qix-): The real `std` stores a TLS handle to the current thread,
	// NOTE(qix-): which is totally valid but the kernel hasn't implemented
	// NOTE(qix-): TLS quite yet. So we do it (slowly) here each time.
	let id = syscall::get!(KERNEL_THREAD_V0, KERNEL_THREAD_V0, 0, syscall::key!("id"))
		.expect("failed to retrieve current thread ID");

	Thread::new(NonZero::new(id).expect("kernel indicated the current thread ID is zero"))
}

/// Cooperatively gives up a timeslice to the OS scheduler.
///
/// This calls the underlying OS scheduler's yield primitive, signaling
/// that the calling thread is willing to give up its remaining timeslice
/// so that the OS may schedule other threads on the CPU.
///
/// A drawback of yielding in a loop is that if the OS does not have any
/// other ready threads to run on the current CPU, the thread will effectively
/// busy-wait, which wastes CPU time and energy.
///
/// # Oro-specific
/// This function **panics** in the rare case the interface is unavailable.
pub fn yield_now() {
	// NOTE(qix-): The real `std` stores a TLS handle to the current thread,
	// NOTE(qix-): which is totally valid but the kernel hasn't implemented
	// NOTE(qix-): TLS quite yet. So we do it (slowly) here each time.
	syscall::set!(
		KERNEL_THREAD_V0,
		KERNEL_THREAD_V0,
		0,
		syscall::key!("yield"),
		0
	)
	.expect("failed to yield current thread");
}
