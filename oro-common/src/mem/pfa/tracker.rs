//! Provides traits and a wrapper around page frame allocators that tracks statistics
//! about memory usage.

use crate::{
	dbg_warn,
	mem::{MemoryRegion, MemoryRegionType, PageFrameAllocate, PageFrameFree},
	Arch,
};
use core::fmt;

/// Provides statistics about the memory usage of the allocator (typically, reflecting
/// that of the system, too).
pub trait PageFrameAllocatorStats {
	/// Gets the number of bytes of memory that are currently in use by the system.
	fn used_memory(&self) -> u64;

	/// Gets the number of bytes of memory that are currently free and available to the system.
	/// This does not include unusable memory regions.
	#[inline]
	fn free_memory(&self) -> u64 {
		self.total_usable_memory() - self.used_memory()
	}

	/// Gets the number of bytes of memory in the system that are unusable.
	/// This **does not** include bad memory.
	fn total_unusable_memory(&self) -> u64;

	/// Gets the number of bytes of "bad" memory in the system.
	/// This is **not** simply unusable memory, but memory explicitly marked
	/// as "bad" by the bootloader. Returns `None` if the bootloader does not
	/// provide this information.
	fn total_bad_memory(&self) -> Option<u64>;

	/// Gets the total amount of memory, including usable, unusable, and bad memory.
	fn total_memory(&self) -> u64 {
		self.total_usable_memory()
			+ self.total_unusable_memory()
			+ self.total_bad_memory().unwrap_or(0)
	}

	/// Gets the total number of bytes of memory that are usable to the system.
	fn total_usable_memory(&self) -> u64;
}

/// A simple memory allocator stats tracker that can be default initialized,
/// or initialized with the base memory stats coming from a memory map.
#[derive(Default, Clone)]
pub struct AllocatorStatsTracker<Alloc> {
	/// The currently allocated number of bytes.
	used_bytes: u64,
	/// The cached total memory size.
	total_memory: u64,
	/// The cached total usable memory size.
	total_usable_memory: u64,
	/// The cached total unusable memory size.
	total_unusable_memory: u64,
	/// The cached total bad memory size.
	total_bad_memory: Option<u64>,
	/// Inner allocator
	inner: Alloc,
}

impl<Alloc> AllocatorStatsTracker<Alloc> {
	/// Creates a new stats tracker based on a memory map.
	///
	/// If the underlying system/bootloader reports "bad" memory,
	/// the `supports_bad_memory` parameter should be set to `true`.
	///
	/// # Panics
	/// Panics if `supports_bad_memory` is false, but bad memory
	/// regions (marked as [`MemoryRegionType::Bad`]) are present
	/// in the memory map.
	#[cold]
	pub fn from_memory_map<
		A: Arch,
		M: MemoryRegion,
		I: IntoIterator<Item = M>,
		const BOOT_IS_USABLE: bool,
	>(
		allocator: Alloc,
		memory_map: I,
		supports_bad_memory: bool,
	) -> Self {
		let mut total_memory = 0;
		let mut total_usable_memory = 0;
		let mut total_unusable_memory = 0;
		let mut total_bad_memory = if supports_bad_memory { Some(0) } else { None };

		for region in memory_map {
			// Align the memory region and then mark any trimmed bytes as unusable
			let original_length = region.length();
			let region = region.aligned(4096);
			let length = region.length();
			total_memory += length;
			let trimmed_memory = original_length - length;

			match region.region_type() {
				MemoryRegionType::Usable => {
					total_usable_memory += length;
					total_unusable_memory += trimmed_memory;
				}
				MemoryRegionType::Unusable => total_unusable_memory += original_length,
				MemoryRegionType::Boot => {
					if BOOT_IS_USABLE {
						total_usable_memory += length;
						total_unusable_memory += trimmed_memory;
					} else {
						total_unusable_memory += original_length;
					}
				}
				MemoryRegionType::Bad => {
					if let Some(total_bad_memory) = total_bad_memory.as_mut() {
						*total_bad_memory += original_length;
					} else {
						dbg_warn!(
							A,
							"allocator_stats",
							"bad memory region provided, but bad memory not marked as supported \
							 by bootloader; marking as unusable"
						);
						total_unusable_memory += original_length;
					}
				}
			}
		}

		Self {
			used_bytes: 0,
			total_memory,
			total_usable_memory,
			total_unusable_memory,
			total_bad_memory,
			inner: allocator,
		}
	}

	/// Adds the given number of bytes to the used memory count.
	#[inline]
	fn add_used_bytes(&mut self, bytes: u64) {
		self.used_bytes += bytes;
	}

	/// Subtracts the given number of bytes from the used memory count.
	#[inline]
	fn sub_used_bytes(&mut self, bytes: u64) {
		self.used_bytes -= bytes;
	}
}

impl<Alloc> PageFrameAllocatorStats for AllocatorStatsTracker<Alloc> {
	#[inline]
	fn used_memory(&self) -> u64 {
		self.used_bytes
	}

	#[inline]
	fn total_unusable_memory(&self) -> u64 {
		self.total_unusable_memory
	}

	#[inline]
	fn total_bad_memory(&self) -> Option<u64> {
		self.total_bad_memory
	}

	#[inline]
	fn total_usable_memory(&self) -> u64 {
		self.total_usable_memory
	}

	#[inline]
	fn total_memory(&self) -> u64 {
		self.total_memory
	}
}

impl<Alloc> fmt::Debug for AllocatorStatsTracker<Alloc> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AllocatorStatsTracker")
			// uses the methods from the stats tracker trait instead of the struct fields
			.field("used", &self.used_memory())
			.field("free", &self.free_memory())
			.field("total", &self.total_memory())
			.field("usable", &self.total_usable_memory())
			.field("unusable", &self.total_unusable_memory())
			.field("bad", &self.total_bad_memory())
			.finish()
	}
}

unsafe impl<Alloc> PageFrameAllocate for AllocatorStatsTracker<Alloc>
where
	Alloc: PageFrameAllocate,
{
	#[inline]
	fn allocate(&mut self) -> Option<u64> {
		let res = self.inner.allocate();
		if res.is_some() {
			self.add_used_bytes(4096);
		}
		res
	}
}

unsafe impl<Alloc> PageFrameFree for AllocatorStatsTracker<Alloc>
where
	Alloc: PageFrameFree,
{
	#[inline]
	unsafe fn free(&mut self, frame: u64) {
		self.inner.free(frame);
		self.sub_used_bytes(4096);
	}
}
