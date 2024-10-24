//! Traits that define how the kernel and preboot initialization routine
//! interacts with the underlying architecture's memory facilities.
//!
//! The entire address space is segmented out into logical regions that
//! are specified by the kernel but ultimately defined by the architecture.
//! The kernel will allocate memory into specific regions, leaving the
//! architecture to properly set up all flags and other necessary controls
//! for those regions to behave as the kernel expects.
use crate::pfa::Alloc;

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! define_registries {
	($($(#[doc = $doc:literal])* $name:ident);* ;) => {
		$(
			$(#[doc = $doc])*
			///
			/// This must be read-write, non-user accessible, and is
			/// **not** executable.
			///
			/// Must **not** overlap with any other segment.
			///
			/// Must NOT span more than 2^63 bytes (on 64-bit architectures)
			/// or 2^31 bytes (on 32-bit architectures).
			///
			/// Must NOT border the beginning or end of an address space.
			fn $name() -> Self::SupervisorSegment;
		)*
	};
}

/// A trait that provides descriptors for the layout of an address space
/// for the underlying architecture.
///
/// # Safety
/// Implementations must ensure that the descriptor is valid for the architecture
/// and that the descriptors do not overlap one another.
///
/// Implementations must also ensure that non-overlapping segments (as are prescribed
/// in the individual segment descriptor method documentation) are safe to be used
/// with copies of the current supervisor space (as returned by the
/// [`AddressSpace::current_supervisor_space`] method), so as to not incur undefined
/// behavior under Rust's safety rules regarding multiple mutable references.
// TODO(qix-): Turn this into a const trait whenever const traits are stabilized.
pub unsafe trait AddressSpace: 'static {
	/// The type of supervisor address space handle that this address space works with.
	type SupervisorHandle: Send + Sized + 'static;
	/// The type of user address space handle that this address space works with.
	type UserHandle: Send + Sized + 'static;

	/// The type of [`AddressSegment`] that this address space
	/// returns for supervisor handle mappings.
	type SupervisorSegment: AddressSegment<Self::SupervisorHandle> + Sized;
	/// The type of [`AddressSegment`] that this address space
	/// returns for userspace handle mappings.
	type UserSegment: AddressSegment<Self::UserHandle> + Sized;

	/// Returns the supervisor address space handle for the current CPU.
	///
	/// # Safety
	/// This function is callable only from the supervisor mode (whatever
	/// that means for the architecture), and must ONLY be called by code
	/// that has exclusive ownership of a segment.
	///
	/// Put another way, calling this function must not result in mapping
	/// any entries that are being mapped into by code with another handle
	/// to the supervisor space (via calling this method).
	///
	/// Further, this function _should_ be considered slow, and only called
	/// when absolutely necessary.
	unsafe fn current_supervisor_space() -> Self::SupervisorHandle;

	/// Creates a new, empty supervisor address space handle.
	///
	/// Returns `None` if any allocation(s) fail.
	fn new_supervisor_space<A>(alloc: &mut A) -> Option<Self::SupervisorHandle>
	where
		A: Alloc;

	/// Creates a new user address space handle based on the given supervisor handle.
	///
	/// The resulting userspace handle should _not_ have any core-local
	/// mappings.
	///
	/// Returns None if any allocation(s) fail.
	fn new_user_space<A>(space: &Self::SupervisorHandle, alloc: &mut A) -> Option<Self::UserHandle>
	where
		A: Alloc;

	/// Duplicates the given supervisor address space handle.
	///
	/// The duplication is performed shallowly, meaning that the new handle
	/// will have its own root page table physical address, but the root mappings
	/// will point to the same physical pages as the original handle.
	///
	/// Returns `None` if any allocation(s) fail.
	fn duplicate_supervisor_space_shallow<A>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
	) -> Option<Self::SupervisorHandle>
	where
		A: Alloc;

	/// Duplicates the given user address space handle.
	///
	/// The duplication is performed shallowly, meaning that the new handle
	/// will have its own root page table physical address, but the root mappings
	/// will point to the same physical pages as the original handle.
	///
	/// Returns None if any allocation(s) fail.
	fn duplicate_user_space_shallow<A>(
		space: &Self::UserHandle,
		alloc: &mut A,
	) -> Option<Self::UserHandle>
	where
		A: Alloc;

	/// Frees and reclaims the user address space handle.
	///
	/// Frees the TOP LEVEL page table, without reclaiming any of the pages
	/// that the page table points to.
	fn free_user_space<A>(space: Self::UserHandle, alloc: &mut A)
	where
		A: Alloc;

	/// Returns the layout descriptor for the module thread segment.
	///
	/// This must be read-write, user accessible, and is
	/// **not** executable.
	fn module_thread_stack() -> Self::UserSegment;

	/// Returns the layout descriptor for the kernel code segment.
	///
	/// This must be read-only, user accessible if the architecture
	/// requires that e.g. interrupts have kernel access, and is executable.
	fn kernel_code() -> Self::SupervisorSegment;

	/// Returns the layout descriptor for the kernel data segment.
	///
	/// This must be read-write, non-user accessible, and is
	/// **not** executable.
	///
	/// **Must overlap with [`Self::kernel_code()`]**
	fn kernel_data() -> Self::SupervisorSegment;

	/// Returns the layout descriptor for the kernel read-only data segment.
	///
	/// This must be read-only, non-user accessible, and is
	/// **not** executable.
	///
	/// **Must overlap with [`Self::kernel_code()`]**
	fn kernel_rodata() -> Self::SupervisorSegment;

	/// Returns the layout descriptor for the kernel's stack segment.
	///
	/// This must be read-write, non-user accessible, and is
	/// **not** executable.
	///
	/// Must **not** overlap with any other segment.
	fn kernel_stack() -> Self::SupervisorSegment;

	/// Returns the core-local layout descriptor for the kernel.
	///
	/// This must be read-write, non-user accessible, and is
	/// **not** executable.
	///
	/// It **must not** overlap with any other segment.
	fn kernel_core_local() -> Self::SupervisorSegment;

	define_registries! {
		/// Returns the layout descriptor for the kernel's Ring registry.
		kernel_ring_registry;
		/// Returns the layout descriptor for the kernel's Ring item registry.
		kernel_ring_item_registry;
		/// Returns the layout descriptor for the kernel's Ring list registry.
		kernel_ring_list_registry;
		/// Returns the layout descriptor for the kernel's Port registry.
		kernel_port_registry;
		/// Returns the layout descriptor for the kernel's Port item registry.
		kernel_port_item_registry;
		/// Returns the layout descriptor for the kernel's Port list registry.
		kernel_port_list_registry;
		/// Returns the layout descriptor for the kernel's Thread registry.
		kernel_thread_registry;
		/// Returns the layout descriptor for the kernel's Thread item registry.
		kernel_thread_item_registry;
		/// Returns the layout descriptor for the kernel's Thread list registry.
		kernel_thread_list_registry;
		/// Returns the layout descriptor for the kernel's Module Instance registry.
		kernel_instance_registry;
		/// Returns the layout descriptor for the kernel's Module Instance item registry.
		kernel_instance_item_registry;
		/// Returns the layout descriptor for the kernel's Module Instance list registry.
		kernel_instance_list_registry;
		/// Returns the layout descriptor for the kernel's Module registry.
		kernel_module_registry;
		/// Returns the layout descriptor for the kernel's Module item registry.
		kernel_module_item_registry;
		/// Returns the layout descriptor for the kernel's Module list registry.
		kernel_module_list_registry;
	}
}

/// An address space segment descriptor.
///
/// Segments are architecture specified
/// ranges of memory whereby physical addresses may be mapped in. Each descriptor
/// has a specific set of flags that are architecture specific for the range.
///
/// Note that ranges may overlap with one another, but the architecture must
/// ensure that the flags are consistent with the kernel's expectations.
///
/// # Safety
/// Implementations must ensure that flags are appropriate for the kernel's expectations
/// of each respective segment, and that any overlapping is consistent with the kernel's
/// expectations.
///
/// Implementations must also ensure that non-overlapping segments do not touch other
/// segments, at all, or dereference any of their e.g. page table entries, or other
/// memory that might also be accessed by copies of the mapper handle. Doing so will
/// incur undefined behavior under Rust's safety rules regarding multiple mutable
/// references.
///
/// Implementations **MUST NOT PANIC** under any circumstance.
pub unsafe trait AddressSegment<Handle: Sized>: Send + 'static {
	/// Returns the range of virtual addresses that this segment covers.
	///
	/// The range is inclusive of the start and end addresses.
	fn range(&self) -> (usize, usize);

	/// Makes the segment shared across all address spaces.
	///
	/// Returns an error if the segment is not empty.
	fn provision_as_shared<A>(&self, space: &Handle, alloc: &mut A) -> Result<(), MapError>
	where
		A: Alloc;

	/// Maps a physical address into the segment at the given virtual address.
	/// Fails if the virtual address is already mapped.
	///
	/// If the caller had allocated the page frame for use and this function fails,
	/// assuming the caller will not retry, it's up to the caller to free the
	/// page frame in order to avoid a memory leak.
	fn map<A>(&self, space: &Handle, alloc: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: Alloc;

	/// Unmaps and reclaims all pages in the segment.
	///
	/// # Safety
	/// Caller must ensure that all reclaimed pages are truly
	/// freeable and not in use by any other address space handle.
	unsafe fn unmap_all_and_reclaim<A>(
		&self,
		space: &Handle,
		alloc: &mut A,
	) -> Result<(), UnmapError>
	where
		A: Alloc;

	/// Maps a physical address into the segment at the given virtual address,
	/// without performing any frees (even if it means a slightly less
	/// efficient implementation).
	///
	/// Note that "nofree" **also means "no-unmap"**. It's unfortunately
	/// not possible to encode that into the type system any better than this.
	/// **This method _may not_ unmap any existing mappings / intermediate tables / etc.**.
	///
	/// Fails if the virtual address is already mapped.
	///
	/// If the caller had allocated the page frame for use and this function fails,
	/// assuming the caller will not retry, it's up to the caller to free the
	/// page frame in order to avoid a memory leak.
	fn map_nofree<A>(
		&self,
		space: &Handle,
		alloc: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: Alloc;

	/// Unmaps a physical address from the segment at the given virtual address.
	/// Fails if the virtual address is not mapped. Returns the physical address
	/// that was previously mapped.
	fn unmap<A>(&self, space: &Handle, alloc: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: Alloc;

	/// Maps the given physical address into the segment at the given virtual address.
	/// If the virtual address is already mapped, the physical address is remapped and the
	/// old physical address is returned.
	fn remap<A>(
		&self,
		space: &Handle,
		alloc: &mut A,
		virt: usize,
		phys: u64,
	) -> Result<Option<u64>, MapError>
	where
		A: Alloc;
}

/// Errors returned by mapping functions
#[derive(Clone, Copy, PartialEq, Debug, Eq)]
pub enum MapError {
	/// The page table entry is already present.
	Exists,
	/// The virtual address passed to the map function
	/// is out of range for the given mapper.
	VirtOutOfRange,
	/// On some architectures, the virtual address must be within
	/// a certain range that is larger than the logical Oro segment
	/// range (e.g. TTBR0/TTBR1 on AArch64). This error indicates that
	/// the virtual address is out of the range of the overall address
	/// space within which the caller is attempting to perform a mapping
	/// operation.
	VirtOutOfAddressSpaceRange,
	/// The virtual address passed to the map function
	/// is not page-aligned.
	VirtNotAligned,
	/// Out of memory.
	OutOfMemory,
}

/// Errors returned by unmapping functions
#[derive(Clone, Copy, PartialEq, Debug, Eq)]
pub enum UnmapError {
	/// No mapping exists at the given virtual address.
	NotMapped,
	/// The virtual address passed to the map function
	/// is out of range for the given mapper.
	VirtOutOfRange,
	/// On some architectures, the virtual address must be within
	/// a certain range that is larger than the logical Oro segment
	/// range (e.g. TTBR0/TTBR1 on AArch64). This error indicates that
	/// the virtual address is out of the range of the overall address
	/// space within which the caller is attempting to perform a mapping
	/// operation.
	VirtOutOfAddressSpaceRange,
	/// The virtual address passed to the map function
	/// is not page-aligned.
	VirtNotAligned,
	/// Out of memory.
	OutOfMemory,
}
