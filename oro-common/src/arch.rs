/// Every architecture that Oro supports must implement this trait.
/// It provides the kernel working knowledge and subroutines that
/// are architecture-specific. It itself is not an object, and an
/// object implementing this is never actually passed on the stack.
/// Instead, all methods are called statically.
pub trait Arch {
	/// Initializes the target CPU.
	/// Note that this is called after bootloading;
	/// be sure that functionality within this method
	/// does not conflict with the bootloader's
	/// initialization routines.
	///
	/// # Safety
	/// This method must be called **exactly once** at the
	/// beginning of the kernel's execution.
	unsafe fn init();

	/// Halts the CPU.
	fn halt() -> !;
}
