//! Common memory management facilities for the Oro Operating System kernel
//! and associated bootloaders.

mod pfa;
mod region;

pub use self::{
	pfa::{
		filo::{FiloPageFrameAllocator, FiloPageFrameManager},
		mmap::MmapPageFrameAllocator,
		AllocatorStatsTracker, PageFrameAllocate, PageFrameAllocatorStats, PageFrameFree,
	},
	region::{MemoryRegion, MemoryRegionType},
};
