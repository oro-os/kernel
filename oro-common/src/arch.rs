//! The [`Arch`] trait is the main interface for architecture-specific
//! implementations in Oro. It provides a set of methods that Oro can
//! call to perform architecture-specific operations, such as disabling
//! interrupts, halting the CPU, and logging messages, along with specifying
//! types for interacting with underlying architecture-specific data (e.g.
//! memory management facilities).
use crate::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{AddressSpace, PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator},
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

	/// The address space layout used by this architecture.
	type AddressSpace: AddressSpace;

	/// A token type for [`Self::prepare_transfer`] to return and [`Self::transfer`]
	/// to consume.
	type TransferToken: Sized;

	/// The ELF class that this architecture uses.
	const ELF_CLASS: ElfClass;

	/// The endianness of the ELF file that this architecture uses.
	const ELF_ENDIANNESS: ElfEndianness;

	/// The ELF machine type that this architecture uses.
	const ELF_MACHINE: ElfMachine;

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

	/// Allows the architecture to prepare the master page tables
	/// for the transfer to the kernel execution context.
	///
	/// This is called once for the primary CPU core, before
	/// the page tables are cloned by the secondary cores.
	///
	/// Any additional memory mappings that are required for the
	/// kernel to execute should be set up here.
	///
	/// # Safety
	/// This method must be called **exactly once** for the primary
	/// CPU core, and **only** by the primary CPU core, before the
	/// secondary cores copy the page tables.
	///
	/// Implementations must take special care to clean up resources
	/// after execution has been transferred to the kernel via the
	/// [`Self::after_transfer()`] method.
	///
	/// Callers of this method **MUST** synchronize with other cores
	/// directly after calling, a transfer is prepared.
	unsafe fn prepare_master_page_tables<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPrimaryConfig,
		A: PageFrameAllocate + PageFrameFree;

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
	/// Callers of this method MUST synchronize with other cores
	/// directly after calling, before performing the transfer.
	///
	/// This method **may panic**.
	///
	/// CALLERS OF THIS METHOD MUST TAKE GREAT CARE THAT NO FURTHER
	/// MEMORY ALLOCATIONS TAKE PLACE AFTER THIS METHOD IS CALLED.
	unsafe fn prepare_transfer<A, C>(
		mapper: <<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) -> Self::TransferToken
	where
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
	/// The boot config virtual address MUST NOT be zero, and MUST be
	/// a valid, readable readable address (as specified by
	/// [`crate::mem::AddressSpace::boot_info()`]) mapped into the
	/// kernel's eventual address space, whereby a cast to a pointer to
	/// [`crate::boot::BootConfig`] is valid and dereferenceable without
	/// invoking undefined behavior.
	///
	/// Implementations MUST NOT derefence the boot config virtual address
	/// as it is not valid in the pre-boot memory map.
	///
	/// This method **must not panic**.
	unsafe fn transfer(
		entry: usize,
		transfer_token: Self::TransferToken,
		boot_config_virt: usize,
		pfa_head: u64,
	) -> !;

	/// Cleans up resources after the transfer has been completed.
	/// Execution is now in the kernel; all architecture-specific
	/// resources that were used to prepare the transfer should be
	/// cleaned up.
	///
	/// # Safety
	/// This method must only be called **once** for each CPU core,
	/// and only after the transfer has been completed.
	///
	/// Implementations MUST NOT affect ANY resources that are not
	/// local to the CPU core being prepared, and must ONLY do what
	/// the architecture documents will occur after the kernel has
	/// taken control.
	unsafe fn after_transfer<A, P>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		translator: &P,
		alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator;

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
