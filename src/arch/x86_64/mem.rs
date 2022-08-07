use ::bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use ::core::mem::size_of;
use ::x86_64::{
	addr::PhysAddr,
	structures::paging::{
		FrameAllocator, FrameDeallocator, OffsetPageTable, PageSize, PageTable, PhysFrame, Size4KiB,
	},
	VirtAddr,
};

static mut KERNEL_ADDRESS_MAPPER: Option<OffsetPageTable> = None;
static mut FRAME_ALLOCATOR: Option<BootInfoFrameAllocator> = None;

struct BootInfoFrameAllocator {
	regions: &'static MemoryRegions,
	mapping_index: usize,
	offset: u64,
	last_unused: u64,
}

impl BootInfoFrameAllocator {
	fn new(memory_regions: &'static MemoryRegions) -> Self {
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
			regions: memory_regions,
			mapping_index: start_index,
			offset: start_offset,
			last_unused: 0,
		}
	}
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
	fn allocate_frame(&mut self) -> Option<PhysFrame> {
		if self.last_unused > 0 {
			static_assert!(Size4KiB::SIZE as usize >= size_of::<usize>());

			let next_unused = self.last_unused;
			self.last_unused = unsafe { *(next_unused as *const u64) };
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
		self.offset += Size4KiB::SIZE;
		return Some(PhysFrame::containing_address(PhysAddr::new(result_offset)));
	}
}

impl FrameDeallocator<Size4KiB> for BootInfoFrameAllocator {
	unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
		let offset = frame.start_address().as_u64();
		*(offset as *mut u64) = self.last_unused;
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
		KERNEL_ADDRESS_MAPPER = Some(init_page_table(phys_offset));
		FRAME_ALLOCATOR = Some(BootInfoFrameAllocator::new(memory_regions));
	}
}
