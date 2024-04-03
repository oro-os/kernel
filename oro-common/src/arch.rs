//! The [`Arch`] trait is the main interface for architecture-specific
//! implementations in Oro. It provides a set of methods that Oro can
//! call to perform architecture-specific operations, such as disabling
//! interrupts, halting the CPU, and logging messages, along with specifying
//! types for interacting with underlying architecture-specific data (e.g.
//! memory management facilities).
use crate::mem::{PhysicalAddressTranslator, PrebootAddressSpace, RuntimeAddressSpace};
use core::fmt;

/// Every architecture that Oro supports must implement this trait.
/// It provides the kernel working knowledge and subroutines that
/// are architecture-specific. It itself is not an object, and an
/// object implementing this is never actually passed on the stack.
/// Instead, all methods are called statically.
///
/// # Safety
/// No method in this trait should ever panic unless it's explicitly
/// documented as safe to do so.
pub unsafe trait Arch {
	/// The type of the interrupt state returned by `fetch_interrupts`
	/// and expected by `restore_interrupts`.
	type InterruptState: Sized + Copy;

	/// The type of [`crate::mem::AddressSpace`] that this architecture implements
	/// for the preboot routine to construct the kernel address space(s).
	///
	/// May be constructed multiple times, one for each CPU core.
	///
	/// Must **not** refer to the current memory map; implementations
	/// _must_ allocate a new memory map for each instance and use
	/// the page frame allocator and translation facilities to create
	/// a brand new address space.
	type PrebootAddressSpace<P: PhysicalAddressTranslator>: PrebootAddressSpace<P> + Sized;

	/// The type of [`crate::mem::AddressSpace`] that this architecture implements
	/// for the kernel itself to mutate and otherwise interact with
	/// the address space for the running execution context.
	///
	/// Constructed once per CPU core at beginning of kernel execution.
	///
	/// Must refer to the current memory map; implementations **must not**
	/// switch the memory map context outside of this type; the kernel
	/// uses this type for **ALL** address space switches.
	type RuntimeAddressSpace: RuntimeAddressSpace + Sized;

	/// Initializes shared resources the target CPU.
	///
	/// # Safety
	/// This method must be called **exactly once** at the
	/// beginning of an execution context (boot stage or kernel),
	/// and **only** by the primary CPU instance.
	unsafe fn init_shared();

	/// Initializes instance-local resources the target CPU.
	///
	/// # Safety
	/// This method must be called **exactly once** at the
	/// beginning of an execution context (boot stage or kernel),
	/// for **all** instances, **only after** `init_shared` has been
	/// called by the primary CPU instance.
	unsafe fn init_local();

	/// Halts the CPU.
	fn halt() -> !;

	/// Disables interrupts for the CPU.
	fn disable_interrupts();

	/// Fetches the current interrupt state.
	fn fetch_interrupts() -> Self::InterruptState;

	/// Restores the current interrupt state, re-enabling
	/// interrupts if they were enabled before.
	fn restore_interrupts(state: Self::InterruptState);

	/// Performs the strongest memory barrier possible on the
	/// target architecture. To the fullest extent possible,
	/// this should ensure that all memory operations are
	/// completed before the barrier returns.
	fn strong_memory_barrier();

	/// Logs a message to the debug logger (typically a serial port).
	///
	/// The message should be newline-terminated for streams,
	/// or otherwise 'chunked' as a single message for non-streams.
	///
	/// This should NOT be used directly; instead, use the `dbg!` et al
	/// macros from the [`oro-common`] crate.
	///
	/// May panic.
	///
	/// # Safety
	/// Only call this function when you are certain that it is safe
	/// to do so. You should probably be using the [`crate::dbg!`] macro instead.
	///
	/// Implementations must ensure
	///
	/// 1. the shared resource, if any, is properly guarded.
	/// 2. no recursive calls to `log` are made (e.g. by calling `dbg!` from within `log`).
	fn log(message: fmt::Arguments);
}
