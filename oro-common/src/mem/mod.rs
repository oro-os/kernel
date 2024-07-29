//! Common memory management facilities for the Oro Operating System kernel
//! and associated bootloaders.

mod mapper;
mod pfa;
mod region;
mod ser2mem;
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

pub(crate) use self::ser2mem::PfaSerializer;
