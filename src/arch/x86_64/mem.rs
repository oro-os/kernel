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

type PageSize = Size4KiB;

const HEAP_CHUNK_SIZE: u64 = 128 * PageSize::SIZE;
const KERNEL_BASE: u64 = 0x8000_0000_0000; // TODO Statically get this from the bootloader config somehow.
const HEAP_BASE: u64 = 0x4000_0000_0000; // Must not conflict with bootloader's physical-memory-offset.

static mut CURRENT_HEAP_BASE: u64 = HEAP_BASE;
static mut KERNEL_PAGE_TABLE: Option<OffsetPageTable> = None;
static mut FRAME_ALLOCATOR: Option<BootInfoFrameAllocator> = None;

static_assert!(HEAP_BASE < KERNEL_BASE);

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

struct BootInfoFrameAllocator {
	phys_offset: u64,
	regions: &'static MemoryRegions,
	mapping_index: usize,
	offset: u64,
	last_unused: u64,
}

impl BootInfoFrameAllocator {
	fn new(phys_offset: u64, memory_regions: &'static MemoryRegions) -> Self {
		let (start_index, start_offset) = if memory_regions.len() == 0 {
			(0, 0)
		} else {
			match memory_regions
				.iter()
				.enumerate()
				.filter(|(_, r)| r.kind == MemoryRegionKind::Usable)
				.next()
			{
				Some((idx, reg)) => (idx, reg.start),
				None => (0, 0),
			}
		};

		Self {
			phys_offset: phys_offset,
			regions: memory_regions,
			mapping_index: start_index,
			offset: start_offset,
			last_unused: u64::MAX,
		}
	}
}

unsafe impl FrameAllocator<PageSize> for BootInfoFrameAllocator {
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
		return Some(PhysFrame::containing_address(PhysAddr::new(result_offset)));
	}
}

impl FrameDeallocator<PageSize> for BootInfoFrameAllocator {
	unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
		let offset = frame.start_address().as_u64();
		*((offset + self.phys_offset) as *mut u64) = self.last_unused;
		self.last_unused = offset;
	}
}

/**
	Must ONLY be called once!
*/
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

/**
	Must ONLY be called once!
*/
unsafe fn init_page_table(phys_offset: VirtAddr) -> OffsetPageTable<'static> {
	OffsetPageTable::new(get_level_4(phys_offset), phys_offset)
}

pub fn init(phys_offset: VirtAddr, memory_regions: &'static MemoryRegions) {
	unsafe {
		KERNEL_PAGE_TABLE = Some(init_page_table(phys_offset));
		FRAME_ALLOCATOR = Some(BootInfoFrameAllocator::new(
			phys_offset.as_u64(),
			memory_regions,
		));
	}
}
