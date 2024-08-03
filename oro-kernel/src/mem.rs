//! Implements wrappers around the common memory management types
//! for use with the global allocator.
//!
//! # Safety
//! The functions in this module are very unsafe, and calling them inappropriately
//! will almost certainly violate Rust's memory safety guarantees and invoke immediate
//! undefined behavior, if not leaking or corrupting kernel memory.
//!
//! Please be very careful when using and/or modifying this module.

// SAFETY(qix-): At face value, this module is very unsafe looking.
// SAFETY(qix-): However, assuming the architecture-specific implementations
// SAFETY(qix-): of address spaces are correct, all unsafe usage is sound.
// SAFETY(qix-):
// SAFETY(qix-): It sets up two global allocators; one for the private (local)
// SAFETY(qix-): kernel heap, and one for the shared kernel heap (which allocates
// SAFETY(qix-): items in such a way that they are visible across cores). There
// SAFETY(qix-): is no other way to do this than to use globals and static mutables,
// SAFETY(qix-): even per Rust's own prescribed patterns.
//
// SAFETY(qix-): In terms of Rust's memory model invariants, this is probably the most
// SAFETY(qix-): unsafe code in the kernel. Please be very scrutinous when modifying
// SAFETY(qix-): anything in this file.
#![allow(clippy::inline_always)]

use buddy_system_allocator::{Heap, LockedHeapWithRescue};
use core::{
	alloc::{GlobalAlloc, Layout},
	mem::MaybeUninit,
};
use oro_common::{
	dbg_warn,
	mem::{
		AddressSegment, AddressSpace, PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator,
	},
	sync::UnfairSpinlock,
	Arch,
};

/// The order of the allocators. See the [`buddy_system_allocator`] crate for more information.
const ORDER: usize = 32;
/// The maximum size of the [`AllocatorState`] type. It will have different
/// sizes based on the architecture, and unfortunately there's no way to extract
/// this size information out at compile time to be used by globals.
///
/// The size _is_, however, checked at compile time.
const ALLOCATOR_STATE_MAX_SIZE: usize = 128;
/// The minimum size of a slab allocation when a heap runs out of memory.
const SLAB_ALLOCATION_MIN_SIZE: usize = 1024 * 16;

/// The global allocator for the private (local) kernel heap.
// TODO(qix-): Make a lock-free version for the private allocator.
// TODO(qix-): There will never be a multi-threaded `Box` access.
#[global_allocator]
static mut PRIVATE_ALLOCATOR: AllocWrapper = AllocWrapper(MaybeUninit::uninit());
/// The allocator state of the private (local) kernel heap.
static mut PRIVATE_ALLOCATOR_STATE: MaybeUninit<AlignedBuffer<ALLOCATOR_STATE_MAX_SIZE>> =
	MaybeUninit::uninit();

/// Initializes the global allocators.
///
/// # Safety
/// This function must be called EXACTLY ONCE per core.
///
/// The `mapper` parameter must be a valid supervisor address space handle
/// for the current core. It must not be used again after this function is called.
///
/// > **NOTE:** On some architectures (e.g. x86_64) the supervisor address space
/// > and user address space are the same. The types are enforced in such a way
/// > that user address space allocations do not conflict with supervisor address
/// > space segments (assuming architecture-specific implementations are correct),
/// > and thus a handle to the user address space can be safely acquired without
/// > concern for the lifetime of the supervisor address space handle.
///
/// The `alloc` parameter must be a valid page frame allocator shared across
/// all cores, and MUST NOT MOVE in memory after this function is called.
pub unsafe fn initialize_allocators<A, P, Alloc>(
	mapper: <<A as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
	translator: P,
	alloc: &'static UnfairSpinlock<Alloc>,
) where
	A: Arch,
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree + 'static,
{
	// SAFETY(qix-): We can "safely" duplicate the handle bytewise here due to Rust
	// SAFETY(qix-): disallowing self-referential structs. We technically use it in
	// SAFETY(qix-): two places, but this module ensures it's never modifying overlapping
	// SAFETY(qix-): memory at the same time, assuming the architecture-specific
	// SAFETY(qix-): implementations of address space layouts are well-formed.
	// SAFETY(qix-):
	// SAFETY(qix-): In terms of Rust's memory model, this is probably the most unsafe
	// SAFETY(qix-): thing in the kernel, but it's also the only way to do it without
	// SAFETY(qix-): a lot of overhead. Please be careful when modifying this code.

	PRIVATE_ALLOCATOR
		.0
		.write(LockedHeapWithRescue::new(rescue_local::<A, P, Alloc>));

	let state = AllocatorState::<A, P, Alloc> {
		mapper,
		cursor: A::AddressSpace::kernel_private_heap().range().0,
		translator,
		alloc,
	};

	// SAFETY(qix-): The references are not used; they're just used to extract type
	// SAFETY(qix-): information automatically. We can safely ignore the advisory.
	#[allow(static_mut_refs)]
	{
		oro_common::util::assertions::assert_fits_within_val(&state, &PRIVATE_ALLOCATOR_STATE);
		oro_common::util::assertions::assert_aligns_within_val(&state, &PRIVATE_ALLOCATOR_STATE);
	}

	// SAFETY(qix-): We can safely write to the static mut here, as we've ensured
	// SAFETY(qix-): that the state fits within the maximum size of the global buffer.
	core::ptr::write_volatile(PRIVATE_ALLOCATOR_STATE.as_mut_ptr().cast(), state);
}

/// Newtype wrapper around an allocator.
#[repr(transparent)]
struct AllocWrapper(MaybeUninit<LockedHeapWithRescue<ORDER>>);

unsafe impl GlobalAlloc for AllocWrapper {
	#[inline(always)]
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		self.0.assume_init_ref().alloc(layout)
	}

	#[inline(always)]
	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		self.0.assume_init_ref().dealloc(ptr, layout);
	}

	#[inline(always)]
	unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
		self.0.assume_init_ref().alloc_zeroed(layout)
	}

	#[inline(always)]
	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		self.0.assume_init_ref().realloc(ptr, layout, new_size)
	}
}

/// Local (private) heap rescue function. Called by the buddy allocator
/// when it runs out of memory.
fn rescue_local<A, P, Alloc>(heap: &mut Heap<ORDER>, layout: &Layout)
where
	A: Arch,
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree + 'static,
{
	// SAFETY(qix-): If this function is being called, we already initialized
	// SAFETY(qix-): the global allocator state.
	let state = unsafe {
		&mut *PRIVATE_ALLOCATOR_STATE
			.as_mut_ptr()
			.cast::<AllocatorState<A, P, Alloc>>()
	};

	let segment = A::AddressSpace::kernel_private_heap();

	// TODO(qix-): Use larger page allocation when they're implemented.
	let start_region = state.cursor;
	let size = (layout.size() + 0xFFF) & !0xFFF;
	let size = size.max(SLAB_ALLOCATION_MIN_SIZE);
	debug_assert_eq!(size & 0xFFF, 0);
	let mut end_region = start_region;

	let num_pages = size >> 12;

	unsafe {
		let mut pfa = state.alloc.lock::<A>();

		for _ in 0..num_pages {
			let Some(page) = pfa.allocate() else {
				dbg_warn!(A, "kernel", "failed to rescue private heap; out of memory");
				break;
			};

			let virt_addr = end_region;
			end_region += 0x1000;

			if let Err(err) =
				segment.map(&state.mapper, &mut *pfa, &state.translator, virt_addr, page)
			{
				pfa.free(page);
				dbg_warn!(
					A,
					"kernel",
					"failed to rescue private heap; map failed: {:?}",
					err
				);
				break;
			}
		}
	}

	if start_region != end_region {
		unsafe {
			heap.add_to_heap(start_region, end_region);
		}
	}
}

/// Stores the state of a local or global allocator.
///
/// # Safety
/// This struct holds a duplicated mapper handle, which is inherently
/// unsafe. However, assuming the architecture-specific implementations
/// of address spaces are correct, it will never map overlapping memory
/// regions and thus never result in otherwise unsafe or undefined
/// behavior.
///
/// It does, however, do a very saucy dance with UB. Please be careful
/// when using or modifying this struct.
struct AllocatorState<A, P, Alloc>
where
	A: Arch,
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree + 'static,
{
	/// The supervisor address space handle for the current CPU core.
	mapper:     <<A as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
	/// The current cursor position in the heap (virtual address).
	/// Starts at the beginning of the heap and grows upwards.
	///
	/// This is the next address that will be allocated.
	cursor:     usize,
	/// The physical address translator.
	translator: P,
	/// The shared page frame allocator.
	alloc:      &'static UnfairSpinlock<Alloc>,
}

/// An aligned byte buffer of fixed length.
///
/// If there is ever an alignment issue on a particular architecture
/// (enforced at compile time) increase the alignment of this struct.
#[repr(align(64))]
struct AlignedBuffer<const N: usize>([u8; N]);
