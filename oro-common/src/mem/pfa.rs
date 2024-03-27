use crate::mem::{MemoryRegion, MemoryRegionType};

pub mod filo;
pub mod mmap;

use core::fmt;

/// A page frame allocator allocates physical memory in units of "page frames".
/// A page frame is a contiguous block of physical memory that is a multiple of
/// the requested page size (e.g. 4 KiB).
///
/// Page allocators that support freeing page frames should also implement the
/// [`PageFrameFree`] trait.
///
/// Consumers of this trait must ensure proper synchronization if the allocator
/// is shared between multiple processors. Implementations **should not** provide any
/// thread safety.
///
/// # Safety
/// The page frame allocator must ensure that all memory accesses are safe and valid
/// during any bookkeeping operations.
///
/// Further, it must ensure that page frame addresses are properly aligned and that
/// no overlapping frames are allocated.
pub unsafe trait PageFrameAllocate {
	/// Allocates a new page frame, returning the physical address of the page frame
	/// that was allocated. If `None` is returned, the system is out of memory.
	///
	/// # Safety
	/// Implementors **must** ensure that the returned frame address
	///
	/// - is page-aligned.
	/// - is not already in use.
	/// - is not in a reserved, bad, or unusable memory region.
	///
	/// Any and all bookkeeping operations must be safe.
	///
	/// Callers must only call this method if the implementing allocator is known to be
	/// in a "good state", for whatever definition of "good" the allocator specifies.
	/// For example, for the `FiloPageFrameAllocator`, this method potentially brings in and
	/// out physical pages from a memory map, and so the caller must ensure that the memory
	/// map is in a consistent state before calling this method.
	unsafe fn allocate(&mut self) -> Option<u64>;
}

/// A page frame allocator that supports freeing page frames.
///
/// # Safety
/// Implementors of this trait must ensure that all memory accesses are safe and valid
/// during any bookkeeping operations.
pub unsafe trait PageFrameFree: PageFrameAllocate {
	/// Frees a page frame.
	///
	/// # Panics
	/// Implementors **must** panic if the passed frame address is not page-aligned.
	///
	/// # Safety
	/// The following **must absolutely remain true**:
	///
	/// 1. Callers **must** ensure the passed frame address is valid and allocated, not in active
	/// use, and is not already freed. Implementors are under no obligation to ensure this.
	///
	/// 2. Callers **must** ensure the passed frame address is not in a reserved or unusable
	/// memory region.
	///
	/// 3. Any and all bookkeeping operations must be safe.
	///
	/// Callers must only call this method if the implementing allocator is known to be
	/// in a "good state", for whatever definition of "good" the allocator specifies.
	/// For example, for the [`self::filo::FiloPageFrameAllocator`], this method potentially brings in and
	/// out physical pages from a memory map, and so the caller must ensure that the memory
	/// map is in a consistent state before calling this method.
	unsafe fn free(&mut self, frame: u64);
}

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
pub struct AllocatorStatsTracker {
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
}

impl AllocatorStatsTracker {
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
		M: MemoryRegion,
		I: IntoIterator<Item = M>,
		const BOOT_IS_USABLE: bool,
	>(
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

			match region.ty() {
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
					if total_bad_memory.is_some() {
						total_bad_memory = Some(total_bad_memory.unwrap() + original_length);
					} else {
						panic!("bad memory region provided, but bad memory is not supported");
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
		}
	}

	/// Adds the given number of bytes to the used memory count.
	#[inline]
	pub fn add_used_bytes(&mut self, bytes: u64) {
		self.used_bytes += bytes;
	}

	/// Subtracts the given number of bytes from the used memory count.
	#[inline]
	pub fn sub_used_bytes(&mut self, bytes: u64) {
		self.used_bytes -= bytes;
	}
}

impl PageFrameAllocatorStats for AllocatorStatsTracker {
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

impl fmt::Debug for AllocatorStatsTracker {
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
