#![expect(unused_imports)]
use ::oro::{id::kernel::iface::THREAD_V0, key, uses};

use crate::thread::Thread;

/// Gets a handle to the thread that invokes it.
///
/// # Oro-specific
/// This function **panics** in the rare case the thread handle cannot be retrieved.
/// This is a temporary measure until the kernel implements TLS.
#[must_use]
pub fn current() -> Thread {
	// NOTE(qix-): The real `std` stores a TLS handle to the current thread,
	// NOTE(qix-): which is totally valid but the kernel hasn't implemented
	// NOTE(qix-): TLS quite yet. So we do it (slowly) here each time.
	uses!(THREAD_V0, key!("id"));
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
	uses!(THREAD_V0, key!("id"));
}
