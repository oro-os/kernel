//! Common memory management facilities for the Oro Operating System kernel
//! and associated bootloaders.

mod mapper;
mod pfa;
mod region;
mod translate;

pub use self::{
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::{
		alloc::{PageFrameAllocate, PageFrameFree},
		filo::{FiloPageFrameAllocator, FiloPageFrameManager},
		tracker::{AllocatorStatsTracker, PageFrameAllocatorStats},
	},
	region::{MemoryRegion, MemoryRegionType},
	translate::{OffsetPhysicalAddressTranslator, PhysicalAddressTranslator},
};
