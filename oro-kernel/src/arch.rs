//! Defines traits that for items that must be provided by the architecture.

use oro_mem::mapper::{AddressSpace, MapError};

/// Implements an architecture for the Oro kernel.
pub trait Arch: Sized + 'static {
	/// The architecture-specific thread handle.
	type ThreadHandle: ThreadHandle<Self>;
	/// The architecture-specific address space layout.
	type AddressSpace: oro_mem::mapper::AddressSpace;
	/// The architecture-specific instance handle.
	type InstanceHandle: InstanceHandle<Self>;
	/// The architecture-specific core handle.
	type CoreHandle: CoreHandle;
	/// The architecture-specific system call handle.
	type SystemCallHandle: SystemCallHandle;
}

/// An architecture-specific thread handle.
///
/// Thread handles are low-level, quasi-subservient controller
/// objects for thread state and architecture-specific operations
/// (such as context switching, construction and cleanup).
///
/// # Safety
/// This trait is inherently unsafe. Implementors must take
/// great care that **all** invariants for **each individual method**
/// are upheld.
pub unsafe trait ThreadHandle<A: Arch>: Sized + Send {
	/// Creates a new thread handle.
	///
	/// Takes the given mapper and uses it to construct **additional** mappings.
	///
	/// # Invariants
	/// Implementors must be aware existing mappings may be present, but assuming
	/// that all invariants of the [`oro_mem::mapper::AddressSpace`] segments are
	/// upheld, no conflicts should arise.
	///
	/// The stack has already been mapped by the kernel. No additional stack preparation
	/// is necessary.
	///
	/// The given entry point is where the next execution should return to.
	///
	/// **All additional mappings created by this method must be reclaimed by
	/// the destructor (in a `Drop` implementation).** Such implementations
	/// **must not** free any memory that was not allocated by this method.
	///
	/// Upon drop or error, the mapper **must be freed without reclaim**
	/// via [`oro_mem::mapper::AddressSpace::free_user_space_handle`].
	fn new(
		mapper: <<A as Arch>::AddressSpace as AddressSpace>::UserHandle,
		stack_ptr: usize,
		entry_point: usize,
	) -> Result<Self, MapError>;

	/// Returns the mapper handle for the thread.
	///
	/// # Invariants
	/// Must return the same mapper handle that was given to the constructor.
	fn mapper(&self) -> &<<A as Arch>::AddressSpace as AddressSpace>::UserHandle;

	/// Migrates the thread to the current core.
	///
	/// # Invariants
	/// Must map in the current kernel and any core-local mappings
	/// into the thread's address space.
	///
	/// Must make the thread ready to be run on the calling core
	/// shortly after being called.
	///
	/// Must be infallible.
	fn migrate(&self);
}

/// Architecture-specific system call frame handle.
///
/// Frames are handed to the kernel to either process or store (if the task must
/// be made dormant) in order to hand _back_ to the architecture for restoration
/// at a later time.
pub trait SystemCallHandle: Sized + Send + Sync {
	/// Returns the opcode for the operation.
	///
	/// Does not need to be validated; the kernel will do that.
	fn opcode(&self) -> oro_sysabi::syscall::Opcode;
	/// Returns the table ID for the operation.
	///
	/// Does not need to be validated; the kernel will do that.
	fn table_id(&self) -> u64;
	/// Returns the entity ID for the operation.
	///
	/// Does not need to be validated; the kernel will do that.
	fn entity_id(&self) -> u64;
	/// Returns the key for the operation.
	fn key(&self) -> u64;
	/// Returns the value for the operation.
	///
	/// Does not need to be validated; the kernel will do that.
	fn value(&self) -> u64;
	/// Sets the return value for the system call.
	fn set_return_value(&mut self, value: u64);
	/// Sets the error code for the system call.
	fn set_error(&mut self, error: oro_sysabi::syscall::Error);
}

/// An architecture-specific instance state handle.
///
/// Instance handles are low-level, quasi-subservient controller
/// objects for module instances and their architecture-specific operations.
///
/// # Safety
/// This trait is inherently unsafe. Implementors must take
/// great care that **all** invariants for **each individual method**
/// are upheld.
pub unsafe trait InstanceHandle<A: Arch>: Sized + Send {
	/// Creates a new instance handle.
	///
	/// Takes the given mapper and uses it to construct **additional** mappings.
	///
	/// # Invariants
	/// Implementors must be aware existing mappings may be present, but assuming
	/// that all invariants of the [`oro_mem::mapper::AddressSpace`] segments are
	/// upheld, no conflicts should arise.
	//
	/// Must make the given instance mapper unique, either by duplicating
	/// all instance-specific RW pages or by implementing COW (copy-on-write) semantics.
	///
	/// **All additional mappings created by this method must be reclaimed by
	/// the destructor (in a `Drop` implementation).** Such implementations
	/// **must not** free any memory that was not allocated by this method.
	///
	/// Upon drop or error, the mapper **must be freed without reclaim**
	/// via [`oro_mem::mapper::AddressSpace::free_user_space_handle`].
	fn new(
		mapper: <<A as Arch>::AddressSpace as AddressSpace>::UserHandle,
	) -> Result<Self, MapError>;

	/// Returns the mapper handle for the instance.
	///
	/// # Invariants
	/// Must return the same mapper handle that was given to the constructor.
	fn mapper(&self) -> &<<A as Arch>::AddressSpace as AddressSpace>::UserHandle;
}

/// A handle to a local core.
///
/// Used primarily to issue timer and other core-wide operations.
///
/// # Safety
/// This trait is inherently unsafe. Implementors must take
/// great care that **all** invariants for **each individual method**
/// are upheld.
pub unsafe trait CoreHandle {
	/// Tells a one-off timer to expire after `ticks`.
	/// The architecture should not transform the number
	/// of ticks unless it has good reason to.
	///
	/// The architecture should call [`crate::scheduler::Scheduler::event_timer_expired()`]
	/// if the timer expires.
	fn schedule_timer(&self, ticks: u32);

	/// Tells the core to cancel any pending timer.
	///
	/// Between this point and a subsequent call to
	/// [`Self::schedule_timer()`], the architecture should
	/// **not** call [`crate::scheduler::Scheduler::event_timer_expired()`].
	fn cancel_timer(&self);
}
