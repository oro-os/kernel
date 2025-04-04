//! Defines traits that for items that must be provided by the architecture.

use oro_mem::{
	alloc::boxed::Box,
	mapper::{AddressSpace, MapError},
};

use crate::{iface::kernel::KernelInterface, table::Table};

mod core;

pub use self::core::*;

/// Implements an architecture for the Oro kernel.
pub trait Arch: Sized + Send + Sync + 'static {
	/// The architecture-specific thread handle.
	type ThreadHandle: ThreadHandle<Self>;
	/// The architecture-specific address space layout.
	type AddressSpace: AddressSpace;
	/// The architecture-specific instance handle.
	type InstanceHandle: InstanceHandle<Self>;
	/// The architecture-specific core handle.
	type CoreHandle: CoreHandle<Self>;

	/// Performs a memory fence.
	fn fence();

	/// Registers any architecture-specific kernel interfaces.
	///
	/// These interfaces MUST be unique, and MUST NOT conflict with
	/// any built-in (cross-architecture) interfaces.
	fn register_kernel_interfaces(table: &mut Table<Box<dyn KernelInterface<Self>>>) {
		let _ = table;
	}
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
