//! The [`Arch`] trait is the main interface for architecture-specific
//! implementations in Oro. It provides a set of methods that Oro can
//! call to perform architecture-specific operations, such as disabling
//! interrupts, halting the CPU, and logging messages, along with specifying
//! types for interacting with underlying architecture-specific data (e.g.
//! memory management facilities).
use crate::{
	interrupt::InterruptHandler,
	mem::{
		mapper::AddressSpace,
		pfa::alloc::{PageFrameAllocate, PageFrameFree},
		translate::PhysicalAddressTranslator,
	},
	preboot::{PrebootConfig, PrebootPlatformConfig},
};
use oro_common_elf::{ElfClass, ElfEndianness, ElfMachine};

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
	///
	/// # Safety
	/// Only halts the current CPU core. It's also a dead-end;
	/// it never returns, and is meant for absolute last-resort
	/// panic / fault modes.
	///
	/// Implementations should refrain from overriding this method's
	/// default implementation unless absolutely necessary.
	#[cold]
	unsafe fn halt() -> ! {
		loop {
			Self::halt_once_and_wait();
			::core::hint::spin_loop();
		}
	}

	/// Halts the CPU once (for whatever definition of "halt" is
	/// appropriate for the architecture) and waits for an interrupt.
	///
	/// This method must **not panic**.
	fn halt_once_and_wait();

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

	/// Allows the architecture to prepare the primary page tables
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
	unsafe fn prepare_primary_page_tables<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPlatformConfig,
		A: PageFrameAllocate + PageFrameFree;

	/// Maps the pages in an address space segment such that they are
	/// shared across all cores.
	///
	/// This is called once for the primary CPU core, after the page
	/// tables have been prepared by the primary core, before being
	/// copied by the secondary cores.
	///
	/// The goal of this method is to ensure that subsequent mappings
	/// within the segment are visible to all other cores.
	///
	/// # Safety
	/// This method must be called **exactly once per registry segment**
	/// on the primary CPU core, and **only** by the primary CPU core,
	/// after the page tables have been prepared, and before the secondaries
	/// copy the page tables.
	unsafe fn make_segment_shared<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		segment: &<Self::AddressSpace as AddressSpace>::SupervisorSegment,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPlatformConfig,
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
		C: PrebootPlatformConfig;

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
	///
	/// The method MUST be called FIRST for the primary core, followed
	/// by a barrier, and then for the secondary core(s).
	unsafe fn after_transfer<A, P>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		translator: &P,
		alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator;

	/// Initializes the interrupt handler for the architecture.
	///
	/// # Safety
	/// This method must be called **exactly once** for each CPU core.
	/// It must be called before any interrupts are enabled, and must
	/// NOT manually enable interrupts afterward (aside for the Arch-provided
	/// interrupt enable/disable mechanisms).
	///
	/// It must be called as soon as possible after the CPU core is
	/// initialized, and before any other interrupt-like events are
	/// expected to be handled.
	///
	/// Implementations must overwrite any existing interrupt handlers
	/// with the new handler, if provided, and must ensure that the
	/// handler is ready to receive interrupts at any time.
	///
	/// Implementations must also enable the appropriate interrupt
	/// masks, enable bits, or other mechanisms to ensure that interrupts
	/// are delivered to the handler after they have been installed.
	///
	/// Put simply, the kernel must be ready to receive interrupts
	/// no later than the invocation of this method.
	///
	/// This method **must not panic**.
	unsafe fn initialize_interrupts<H: InterruptHandler>();
}
