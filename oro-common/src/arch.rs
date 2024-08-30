//! The [`Arch`] trait is the main interface for architecture-specific
//! implementations in Oro. It provides a set of methods that Oro can
//! call to perform architecture-specific operations, such as disabling
//! interrupts, halting the CPU, and logging messages, along with specifying
//! types for interacting with underlying architecture-specific data (e.g.
//! memory management facilities).
use crate::mem::mapper::AddressSpace;
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

	/// Performs the strongest memory barrier possible on the
	/// target architecture. To the fullest extent possible,
	/// this should ensure that all memory operations are
	/// completed before the barrier returns.
	fn strong_memory_barrier();
}
