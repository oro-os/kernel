//! Traits that define how the kernel and preboot initialization routine
//! interacts with the underlying architecture's memory facilities.
//!
//! The entire address space is segmented out into logical regions that
//! are specified by the kernel but ultimately defined by the architecture.
//! The kernel will allocate memory into specific regions, leaving the
//! architecture to properly set up all flags and other necessary controls
//! for those regions to behave as the kernel expects.
#![allow(clippy::inline_always)]

use super::{PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator};

/// A const trait that provides descriptors for the layout of an address space
/// for the underlying architecture.
///
/// # Safety
/// Implementations must ensure that the descriptor is valid for the architecture
/// and that the descriptors do not overlap one another.
// TODO(qix-): Turn this into a const trait whenever const traits are stabilized.
pub unsafe trait AddressSpaceLayout {
	/// The descriptor type that is passed to mapper methods to create
	/// address space segments.
	type Descriptor: Sized + 'static;

	/// Returns the layout descriptor for the kernel code segment.
	///
	/// This must be read-only, user accessible if the architecture
	/// requires that e.g. interrupts have kernel access, and is executable.
	fn kernel_code() -> Self::Descriptor;

	/// Returns the layout descriptor for the kernel data segment.
	///
	/// This must be read-write, non-user accessible, and is
	/// **not** executable.
	///
	/// **Must overlap with [`Self::kernel_code()`]**
	fn kernel_data() -> Self::Descriptor;

	/// Returns the layout descriptor for the kernel read-only data segment.
	///
	/// This must be read-only, non-user accessible, and is
	/// **not** executable.
	///
	/// **Must overlap with [`Self::kernel_code()`]**
	fn kernel_rodata() -> Self::Descriptor;
}

/// Base trait for all address space mappers.
///
/// # Safety
/// Architecture implementations **MUST** use the same address space layout
/// for all address space mappers, and **MUST** ensure that the layout is valid
/// for the architecture.
pub unsafe trait AddressSpace {
	/// The layout used by the address space.
	/// Must be **identical** for all address spaces implementations.
	type Layout: AddressSpaceLayout;
}

/// An address space mapper that provides supervisor mapper segments.
pub trait SupervisorAddressSpace: AddressSpace {
	/// The type of [`SupervisorAddressSegment`] that this address space returns.
	type Segment<'a>: SupervisorAddressSegment
	where
		Self: 'a;

	/// Creates a supervisor segment for the given [`AddressSpaceLayout::Descriptor`].
	fn for_supervisor_segment(
		&self,
		descriptor: <Self::Layout as AddressSpaceLayout>::Descriptor,
	) -> Self::Segment<'_>;

	/// Returns a supervisor segment for the kernel code segment.
	#[inline(always)]
	fn kernel_code(&self) -> Self::Segment<'_> {
		self.for_supervisor_segment(Self::Layout::kernel_code())
	}

	/// Returns a supervisor segment for the kernel data segment.
	#[inline(always)]
	fn kernel_data(&self) -> Self::Segment<'_> {
		self.for_supervisor_segment(Self::Layout::kernel_data())
	}

	/// Returns a supervisor segment for the kernel read-only data segment.
	#[inline(always)]
	fn kernel_rodata(&self) -> Self::Segment<'_> {
		self.for_supervisor_segment(Self::Layout::kernel_rodata())
	}
}

/// An address space mapper that allocates a new address space from a page frame allocator
/// and uses a [`PhysicalAddressTranslator`] to translate physical addresses from the allocator
/// to virtual addresses for reading/writing. Used by the preboot initialization routine.
///
/// # Safety
/// Implementations **must** make sure to invalidate cache entries after each mapping and unmapping,
/// for whatever definition of "invalidate" is appropriate for the architecture.
pub unsafe trait PrebootAddressSpace<P>: SupervisorAddressSpace + Sized
where
	P: PhysicalAddressTranslator,
{
	/// Allocates a new address space from the given page frame allocator and physical address translator.
	///
	/// Returns `None` if the page frame allocator is out of memory.
	fn new<A>(allocator: &mut A, translator: P) -> Option<Self>
	where
		A: PageFrameAllocate;
}

/// An [`AddressSpace`] used by the kernel at runtime to map and unmap virtual addresses
/// into the current execution context.
///
/// # Safety
/// Implementations must make sure never to return `true` from `is_active` if the address space
/// is not currently active.
///
/// Implementations must also **not** invalidate cache entries on mapping and unmapping when
/// possible for the architecture, and instead optimize to invalidate them when the address space
/// is switched.
///
/// Implementations must also ensure that the cache is valid after `make_active` completes.
///
/// Implementations should be aware that modifications to or queries of the address space will NOT be
/// performed when the address space is not active. Implementations are encouraged to panic if
/// this is the case in debug builds to ensure this.
///
/// An address space handle must always be active at any given time. Switching handles MUST NOT
/// be done in a way that leaves the address space in an invalid state or that would cause the kernel
/// code/data mappings to be corrupted (except for when explicitly allowed by an address space segment
/// getter).
// TODO(qix-): The above safety requirements are not currently enforced by the trait system,
// TODO(qix-): which is unfortunate since they are critical for correct operation. Doesn't make
// TODO(qix-): me very comfortable that that's the case, so I might rework this in the future.
pub unsafe trait RuntimeAddressSpace: SupervisorAddressSpace + Sized {
	/// The type of 'handle' that is used to refer to separate page tables.
	/// This is typically a reference to the top-level page table base address,
	/// but might be more involved if the memory system for the architecture
	/// is more complex.
	type AddressSpaceHandle: Sized + Copy;

	/// Gets the currently active address space for the CPU.
	///
	/// # Safety
	/// This function must ONLY be called **ONCE** per core by the kernel.
	unsafe fn take() -> Self;

	/// Makes the address space for the given handle active.
	///
	/// Returns the previous handle that was active.
	///
	/// # Safety
	/// This function will almost definitely change the memory contents
	/// of entire swathes of addresses, so callers must take care to
	/// instruct the compiler appropriately to avoid optimizations that
	/// might make incorrect assumptions about writes and reads.
	///
	/// Callers must **only** pass handles that were returned by [`Self::handle()`].
	unsafe fn make_active(&mut self, handle: Self::AddressSpaceHandle) -> Self::AddressSpaceHandle;

	/// Gets the currently active handle for the address space.
	fn handle(&self) -> Self::AddressSpaceHandle;
}

/// A trait for mapping and unmapping virtual addresses to physical page frames
/// within a supervisor segment of an address space.
///
/// The distinction is typically due to the TLB/cache invalidation strategy
/// whereby supervisor mappings remain valid across context switches, while
/// user mappings are invalidated, either manually or via a mechanism such as
/// address space IDs.
///
/// # Safety
/// Implementations must ensure that cache entries are invalidated after (un)mapping,
/// for whatever definition of invalidation is appropriate for the architecture.
pub unsafe trait SupervisorAddressSegment {
	/// Maps a virtual address to a physical address.
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree;

	/// Unmaps a virtual address, returning the page frame that was mapped.
	fn unmap<A>(&mut self, allocator: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree;
}

/// Errors returned by mapping functions
#[derive(Clone, Copy, PartialEq, Debug, Eq)]
pub enum MapError {
	/// The page table entry is already present.
	Exists,
	/// The virtual address passed to the map function
	/// is out of range for the given mapper.
	VirtOutOfRange,
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
	/// The virtual address passed to the map function
	/// is not page-aligned.
	VirtNotAligned,
	/// Out of memory.
	OutOfMemory,
}
