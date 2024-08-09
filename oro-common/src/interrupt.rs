//! Provides a common trait for forwarding interrupts or
//! other interrupt-like events to the kernel.

/// A trait for forwarding interrupts or other interrupt-like events to the kernel.
///
/// # For Architectures
/// This trait is provided to the architecture via the
/// [`crate::arch::Arch::initialize_interrupts()`] method.
///
/// All handlers must be used by _some_ architecture-specific
/// mechanism, except if specified otherwise.
///
///
/// # Safety
/// This trait must only be implemented by the kernel. The implementation
/// must be ready to receive these interrupts at any time, even in the middle
/// of critical code.
///
/// Implementations must be aware that almost everything about their
/// typical 'world view' of the environment is likely to be undefined.
/// No assumptions about the stack, non-supervisor memory, etc. can be
/// made except when the interrupt handler has explicitly provided
/// that information to the kernel via arguments to the methods in this
/// trait.
///
/// Implementations may not panic, invoke side effects aside from
/// simple state changes to well-known locations in memory, or otherwise
/// cause undefined behavior. I/O is highly discouraged. The implementation
/// should keep the interrupt methods as short as possible.
pub unsafe trait InterruptHandler {
	/// The target tick rate for the [`InterruptHandler::handle_tick()`]
	/// interrupt handler. This is the number of ticks per second that the
	/// architecture should _try_ to achieve. There is, however,
	/// no hard guarantee that this rate will be achieved, and
	/// no way for the kernel to verify this. Note that deviations
	/// from this rate in the 'faster' direction may cause performance
	/// degradation due to more frequent pre-emption and context switching,
	/// while deviations in the 'slower' direction may cause sluggishness
	/// in the system, missed deadlines, or other timing-related issues.
	const TARGET_TICK_RATE_HZ: u64;

	/// Handles the main tick interrupt.
	///
	/// While not enforced, this function should be called at a rate
	/// of `TARGET_TICK_RATE_HZ` times per second, or as close to it
	/// as possible.
	///
	/// **Invocation of this method is required by the architecture.**
	///
	/// # Safety
	/// Callers must ensure that no subsequent calls to this function
	/// occur until the previous call has returned.
	unsafe fn handle_tick();
}
