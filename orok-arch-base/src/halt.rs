//! Traits for defining halt functionality for each architecture.

/// Implements halting for a platform. Used in panic scenarios.
pub trait Halt {
	/// Halts the CPU once.
	///
	/// Permanently halting is performed in [`Halt::halt()`],
	/// which by default calls this function in a loop.
	///
	/// # Safety
	/// Caller must expect the CPU to stop, perhaps forever.
	/// There is no guarantee about when (or if) this function
	/// ever returns.
	unsafe fn halt_once();

	/// Halts the CPU forever.
	///
	/// # Safety
	/// Caller must expect the CPU core to permanently and
	/// irrecoverably halt, forever, until the system
	/// is reset.
	unsafe fn halt() -> ! {
		loop {
			// SAFETY: Safety constraints offloaded to caller.
			unsafe {
				Self::halt_once();
			}
		}
	}
}
