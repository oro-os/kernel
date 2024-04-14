//! The [`Arch`] trait is the main interface for architecture-specific
//! implementations in Oro. It provides a set of methods that Oro can
//! call to perform architecture-specific operations, such as disabling
//! interrupts, halting the CPU, and logging messages, along with specifying
//! types for interacting with underlying architecture-specific data (e.g.
//! memory management facilities).
use crate::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{
		PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator, PrebootAddressSpace,
		RuntimeAddressSpace,
	},
	PrebootConfig, PrebootPrimaryConfig,
};
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

	/// A token type for [`Self::prepare_transfer`] to return and [`Self::transfer`]
	/// to consume.
	type TransferToken: Sized;

	/// The ELF class that this architecture uses.
	const ELF_CLASS: ElfClass;

	/// The endianness of the ELF file that this architecture uses.
	const ELF_ENDIANNESS: ElfEndianness;

	/// The ELF machine type that this architecture uses.
	const ELF_MACHINE: ElfMachine;

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

	/// Prepares the CPU for an execution control transfer.
	///
	/// This is used only when the preboot stage is about to transfer
	/// control to the kernel. It should perform any necessary
	/// setup to the *existing* execution context to prepare for
	/// the kernel to begin executing.
	///
	/// As such, this call is given a page frame allocator reference,
	/// whereas the [`Arch::transfer()`] method is not.
	///
	/// Returns a token that is passed to the transfer function.
	///
	/// # Safety
	/// This method is called **exactly once** for each CPU core -
	/// both primary and secondary. It is immediately followed by
	/// a call to [`Arch::transfer()`].
	///
	/// It is always called for the primary _first_; other cores
	/// must wait for the primary to finish before they are called.
	///
	/// Implementations MUST NOT affect ANY resources that are not
	/// local to the CPU core being prepared.
	///
	/// However, it **MAY** affect the local memory map (e.g. to
	/// map in transfer stubs), assuming it does not affect other cores
	/// or the remaining execution of the preboot stage.
	///
	/// This method must ensure that the call to `transfer()` will
	/// succeed.
	///
	/// This method must not affect the commons library from writing
	/// to the boot protocol area.
	///
	/// This method **may panic**.
	unsafe fn prepare_transfer<P, A, C>(
		mapper: Self::PrebootAddressSpace<P>,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) -> Self::TransferToken
	where
		P: PhysicalAddressTranslator,
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig;

	/// Transfers control to the kernel.
	///
	/// The entry point is given as a virtual address in the target address
	/// space.
	///
	/// # Safety
	/// This method is called **exactly once** for each CPU core -
	/// both primary and secondary. It is immediately preceded by
	/// a call to [`Arch::prepare_transfer()`].
	///
	/// Implementations MUST NOT affect ANY resources that are not
	/// local to the CPU core being prepared.
	///
	/// This method **must not panic**.
	unsafe fn transfer(entry: usize, transfer_token: Self::TransferToken) -> !;

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
