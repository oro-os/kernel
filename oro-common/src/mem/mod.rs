//! Common memory management facilities for the Oro Operating System kernel
//! and associated bootloaders.

mod mapper;
mod pfa;
mod region;
mod translate;

pub(crate) use self::pfa::pof_mmap::PanicOnFreeAllocator;
pub use self::{
	mapper::{
		AddressSpace, AddressSpaceLayout, CloneToken, MapError, PrebootAddressSpace,
		RuntimeAddressSpace, SupervisorAddressSegment, SupervisorAddressSpace, UnmapError,
	},
	pfa::{
		filo::{FiloPageFrameAllocator, FiloPageFrameManager},
		mmap::MmapPageFrameAllocator,
		AllocatorStatsTracker, PageFrameAllocate, PageFrameAllocatorStats, PageFrameFree,
	},
	region::{MemoryRegion, MemoryRegionType},
	translate::{OffsetPhysicalAddressTranslator, PhysicalAddressTranslator},
};
