//! Traits that define how the kernel and preboot initialization routine
//! interacts with the underlying architecture's memory facilities.
//!
//! The entire address space is segmented out into logical regions that
//! are specified by the kernel but ultimately defined by the architecture.
//! The kernel will allocate memory into specific regions, leaving the
//! architecture to properly set up all flags and other necessary controls
//! for those regions to behave as the kernel expects.
#![allow(clippy::inline_always)]

use crate::mem::{PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator};

/// A const trait that provides descriptors for the layout of an address space
/// for the underlying architecture.
///
/// # Safety
/// Implementations must ensure that the descriptor is valid for the architecture
/// and that the descriptors do not overlap one another.
// TODO(qix-): Turn this into a const trait whenever const traits are stabilized.
pub unsafe trait AddressSpace {
	/// The type of supervisor address space handle that this address space works with.
	type SupervisorHandle: Sized + 'static;

	/// The type of [`AddressSegment`] that this address space returns.
	type SupervisorSegment: AddressSegment<Self::SupervisorHandle> + Sized + 'static;

	/// Returns the supervisor address space handle for the current CPU.
	///
	/// # Safety
	/// This function must ONLY be called ONCE.
	unsafe fn current_supervisor_space<P>(translator: &P) -> Self::SupervisorHandle
	where
		P: PhysicalAddressTranslator;

	/// Creates a new, empty supervisor address space handle.
	///
	/// Returns `None` if any allocation(s) fail.
	fn new_supervisor_space<A, P>(alloc: &mut A, translator: &P) -> Option<Self::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator;

	/// Duplicates the given supervisor address space handle.
	/// The duplication is performed shallowly, meaning that the new handle
	/// will have its own root page table physical address, but the root mappings
	/// will point to the same physical pages as the original handle.
	///
	/// Returns `None` if any allocation(s) fail.
	fn duplicate_supervisor_space_shallow<A, P>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
		translator: &P,
	) -> Option<Self::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator;

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

	/// Returns the layout descriptor for the direct map of physical addresses.
	///
	/// This must be read-write, non-user accessible, and is
	/// **not** executable.
	///
	/// Must **not** overlap with any other segment.
	fn direct_map() -> Self::SupervisorSegment;
}

/// An address space segment descriptor. Segments are architecture specified
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
pub unsafe trait AddressSegment<Handle: Sized> {
	/// Returns the range of virtual addresses that this segment covers.
	fn range(&self) -> (usize, usize);

	/// Maps a physical address into the segment at the given virtual address.
	/// Fails if the virtual address is already mapped.
	fn map<A, P>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
		phys: u64,
	) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator;

	/// Unmaps a physical address from the segment at the given virtual address.
	/// Fails if the virtual address is not mapped. Returns the physical address
	/// that was previously mapped.
	fn unmap<A, P>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
	) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator;

	/// Maps the given physical address into the segment at the given virtual address.
	/// If the virtual address is already mapped, the physical address is remapped and the
	/// old physical address is returned.
	fn remap<A, P>(
		&self,
		space: &Handle,
		alloc: &mut A,
		translator: &P,
		virt: usize,
		phys: u64,
	) -> Result<Option<u64>, MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator;
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
