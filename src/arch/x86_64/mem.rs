//! Contains the Physical Memory Mapper (PMM), Page Frame Allocator (PFA),
//! and the kernel heap allocator.
//!
//! The Physical Memory Mapper (PMM) is the mechanism that commits allocated
//! pages into the kernel's address space. This typically only happens when
//! the heap allocator has invoked its rescue ("allocation too large" or
//! "out of memory") function, in which case the PFA allocates a new page
//! and the PMM maps it into kernel address space.
//!
//! The Page Frame Allocator (PFA) reads the memory map provided by the
//! bootloader and iterates over all unallocated usable pages in physical
//! memory. When the PFA is given a page to release, that page is added
//! to a linked list of all other pages, which are then handed back to
//! the application first. This means that, if the linked list is empty,
//! and no more pages are available from the memory map, then the system
//! has completely run out of memory.
//!
//! The kernel heap allocator uses the PFA to allocate chunks of frames
//! and create buddy allocators out of them. The number of pages (frames)
//! allocated per buddy arena is specified by [`HEAP_CHUNK_SIZE`]. The heap
//! allocator then maps those pages into kernel memory via the PMM, and
//! passes back the base address of the allocation to the caller.
//!
//! The kernel heap allocator is used as the global Rust runtime allocator
//! for the kernel.

use ::bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use ::buddy_system_allocator::{Heap, LockedHeapWithRescue};
use ::core::mem::size_of;
use ::x86_64::{
	addr::PhysAddr,
	structures::paging::{
		FrameAllocator, FrameDeallocator, Mapper, OffsetPageTable, Page, PageSize as _PageSize,
		PageTable, PageTableFlags, PhysFrame, Size4KiB,
	},
	VirtAddr,
};
use alloc::alloc::Layout;

/// The page size to use for all allocations.
///
/// NOTE: This is a temporary fix and will eventually go away when
/// huge page support is added.
type PageSize = Size4KiB;

/// The number of bytes to allocate for new buddy arenas when the
/// global allocator panics due to being out of space.
const HEAP_CHUNK_SIZE: u64 = 128 * PageSize::SIZE;
/// The base address of the linear memory map set up by the bootloader.
///
/// FIXME: This is entirely unnecessary. The bootloader passes this in
/// at runtime. We should be using that value instead of the one here.
#[deprecated(note = "please use the physical offset passed in by the bootloader")]
const KERNEL_BASE: u64 = 0x8000_0000_0000;
/// The base address of kernel heap storage. New heap arenas are alloated
/// starting from this address.
const HEAP_BASE: u64 = 0x4000_0000_0000; // Must not conflict with bootloader's physical-memory-offset.

/// Where the next heap arena allocation will be based.
static mut CURRENT_HEAP_BASE: u64 = HEAP_BASE;
/// The kernel page table used to map frames into kernel address space.
static mut KERNEL_PAGE_TABLE: Option<OffsetPageTable> = None;
/// The Page Frame Allocator (PFA) instance used to shell out new, unused
/// memory frames given the bootloader memory map.
static mut FRAME_ALLOCATOR: Option<BootInfoFrameAllocator> = None;

static_assert!(HEAP_BASE < KERNEL_BASE);

/// The global allocator instance, using a buddy allocation system with
/// rescue function that allocates a new heap arena in the kernel address
/// space.
#[global_allocator]
static KERNEL_HEAP_ALLOCATOR: LockedHeapWithRescue<32> = LockedHeapWithRescue::new(
	|heap: &mut Heap<32>, layout: &Layout| {
		unsafe {
			// FIXME: This is avoidable with some clever calculations here.
			// FIXME: As long as this is fixed prior to heap_end being calculated,
			// FIXME: then we can avoid this class of panic. It's not super important
			// FIXME: for now, however.
			if layout.size() as u64 > HEAP_CHUNK_SIZE {
				println!("CRITICAL WARNING: allocation size exceeded kernel heap chunk size! KERNEL WILL PANIC!");
				return;
			}

			let heap_start = VirtAddr::new(CURRENT_HEAP_BASE);
			let heap_end = heap_start + HEAP_CHUNK_SIZE - 1u64;

			if heap_end.as_u64() >= KERNEL_BASE {
				println!("CRITICAL WARNING: kernel heap has grown into the base kernel image zone! KERNEL WILL PANIC!");
				return;
			}

			let page_range = {
				let heap_start_page = Page::containing_address(heap_start);
				let heap_end_page = Page::containing_address(heap_end);
				Page::range_inclusive(heap_start_page, heap_end_page)
			};

			let frame_allocator = FRAME_ALLOCATOR.as_mut().unwrap();
			let mapper = KERNEL_PAGE_TABLE.as_mut().unwrap();

			for page in page_range {
				if let Some(frame) = frame_allocator.allocate_frame() {
					let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
					if let Ok(tlb_entry) = mapper.map_to(page, frame, flags, frame_allocator) {
						tlb_entry.flush();
					} else {
						return; // Will inevitably cause the heap allocator to fail and the kernel to stall.
					}
				} else {
					return; // Will inevitably cause the heap allocator to fail and the kernel to stall.
				}
			}

			heap.add_to_heap(
				heap_start.as_u64() as usize,
				(heap_end.as_u64() + 1u64) as usize,
			);

			CURRENT_HEAP_BASE += HEAP_CHUNK_SIZE;
		}
	},
);

/// A Page Frame Allocator (PFA) that pulls frames from a
/// [`bootloader::boot_info::MemoryRegions`] reference.
struct BootInfoFrameAllocator {
	/// The physical offset of the linear physical memory map established by the bootloader
	phys_offset: u64,
	/// The non-offset-adjusted physical memory map reference provided by the bootloader
	regions: &'static MemoryRegions,
	/// The current [`bootloader::boot_info::MemoryRegion`] offset in the [`Self::regions`] slice
	/// currently being iterated
	mapping_index: usize,
	/// The offset within the current [`Self::regions`] mapping (determined by [`Self::mapping_index`])
	/// that is being iterated
	offset: u64,
	/// The physical offset of the most recently freed (and not since re-allocated) memory
	/// frame. Set to [`u64::MAX`] if no memory frames are available for re-use. In such a
	/// case, all physical memory has been allocated when the [`Self::mapping_index`] is greater
	/// than or equal to the number of [`Self::regions`] entries.
	last_unused: u64,
}

impl BootInfoFrameAllocator {
	/// Creates a new instance of `BootInfoFrameAllocator`.
	///
	/// # Arguments
	///
	/// * `phys_offset` - The offset of the linear physical memory map established
	///   by the bootloader
	///
	/// * `memory_regions` - The non-offset-adjusted physical memory map provided
	///   by the bootloader
	fn new(phys_offset: u64, memory_regions: &'static MemoryRegions) -> Self {
		let (start_index, start_offset) = if memory_regions.len() == 0 {
			(0, 0)
		} else {
			match memory_regions
				.iter()
				.enumerate()
				.find(|(_, r)| r.kind == MemoryRegionKind::Usable)
			{
				Some((idx, reg)) => (idx, reg.start),
				None => (0, 0),
			}
		};

		Self {
			phys_offset,
			regions: memory_regions,
			mapping_index: start_index,
			offset: start_offset,
			last_unused: u64::MAX,
		}
	}
}

unsafe impl FrameAllocator<PageSize> for BootInfoFrameAllocator {
	/// Allocate a new, unused frame of physical memory.
	/// Returns [`None`] in the case that all physical memory
	/// has been exhausted.
	fn allocate_frame(&mut self) -> Option<PhysFrame> {
		if self.last_unused != u64::MAX {
			static_assert!(PageSize::SIZE as usize >= size_of::<usize>());

			let next_unused = self.last_unused;
			self.last_unused = unsafe { *((next_unused + self.phys_offset) as *const u64) };
			return Some(PhysFrame::containing_address(PhysAddr::new(next_unused)));
		}

		if self.mapping_index >= self.regions.len() {
			return None;
		}

		loop {
			let region = &self.regions[self.mapping_index];

			if region.kind == MemoryRegionKind::Usable
				&& self.offset >= region.start
				&& self.offset < region.end
			{
				break;
			}

			self.mapping_index += 1;
			if self.mapping_index < self.regions.len() {
				self.offset = self.regions[self.mapping_index].start;
			} else {
				return None;
			}
		}

		let result_offset = self.offset;
		self.offset += PageSize::SIZE;
		Some(PhysFrame::containing_address(PhysAddr::new(result_offset)))
	}
}

impl FrameDeallocator<PageSize> for BootInfoFrameAllocator {
	/// Release a frame to be used again in the next allocation.
	///
	/// # Arguments
	///
	/// * `frame` - The physical frame to release.
	///
	/// # Unsafe
	///
	/// It is **IMPERATIVE** that the released frame has not
	/// been previously released ("double free"). This presents
	/// a security concern for the entire system.
	///
	/// TODO: In debug builds, write a canary value to the page
	/// when released and check for it again at the beginning
	/// of the method. There will be cases of false positives,
	/// but the chance of that happening are extremely low.
	/// Log them out to the serial line.
	unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
		let offset = frame.start_address().as_u64();
		debug_assert!((offset % PageSize::SIZE) == 0);
		*((offset + self.phys_offset) as *mut u64) = self.last_unused;
		self.last_unused = offset;
	}
}

/// Returns the level 4 page table given the linear physical memory
/// map address provided by the bootloader.
///
/// # Unsafe
///
/// This function **MUST** only be called once. Calling this function
/// multiple times invokes immediate undefined behavior.
///
/// Debug builds enforce this constraint.
unsafe fn get_level_4(phys_offset: VirtAddr) -> &'static mut PageTable {
	#[cfg(debug_assertions)]
	{
		use core::sync::atomic::{AtomicBool, Ordering};
		static CALLED: AtomicBool = AtomicBool::new(false);
		if CALLED
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_err()
		{
			panic!("must only call get_level_4() once!");
		}
	}

	use ::x86_64::registers::control::Cr3;
	let (level_4, _) = Cr3::read();
	let phys = level_4.start_address();
	let virt = phys_offset + phys.as_u64();
	let page_table: *mut PageTable = virt.as_mut_ptr();
	&mut *page_table
}

/// Initializes the kernel page table instance.
///
/// # Unsafe
///
/// This function **MUST** only be called once. Calling this function
/// multiple times invokes immediate undefined behavior.
///
/// Debug builds enforce this constraint.
unsafe fn init_page_table(phys_offset: VirtAddr) -> OffsetPageTable<'static> {
	OffsetPageTable::new(get_level_4(phys_offset), phys_offset)
}

/// Initializes the Oro memory management facilities for the x86_64 architecture.
///
/// # Arguments
///
/// * `phys_offset` - The offset of the linear physical memory map established
///   by the bootloader
///
/// * `memory_regions` - The non-offset-adjusted physical memory map provided
///   by the bootloader
///
/// # Unsafe
///
/// This function **MUST** only be called once. Calling this function
/// multiple times invokes immediate undefined behavior.
///
/// Debug builds enforce this constraint.
pub fn init(phys_offset: VirtAddr, memory_regions: &'static MemoryRegions) {
	unsafe {
		KERNEL_PAGE_TABLE = Some(init_page_table(phys_offset));
		FRAME_ALLOCATOR = Some(BootInfoFrameAllocator::new(
			phys_offset.as_u64(),
			memory_regions,
		));
	}
}
