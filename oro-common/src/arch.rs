use core::fmt;

/// Every architecture that Oro supports must implement this trait.
/// It provides the kernel working knowledge and subroutines that
/// are architecture-specific. It itself is not an object, and an
/// object implementing this is never actually passed on the stack.
/// Instead, all methods are called statically.
pub trait Arch {
	/// Initializes shared resources the target CPU.
	///
	/// # Safety
	/// This method must be called **exactly once** at the
	/// beginning of the kernel's execution, and **only**
	/// by the primary CPU instance.
	unsafe fn init_shared();

	/// Initializes instance-local resources the target CPU.
	///
	/// # Safety
	/// This method must be called **exactly once** at the
	/// beginning of the kernel's execution, for **all**
	/// instances, **only after** `init_shared` has been
	/// called by the primary CPU instance.
	unsafe fn init_local();

	/// Halts the CPU.
	fn halt() -> !;

	/// Logs a message to the debug logger (typically a serial port).
	///
	/// The message should be newline-terminated for streams,
	/// or otherwise 'chunked' as a single message for non-streams.
	fn log(message: fmt::Arguments);
}
