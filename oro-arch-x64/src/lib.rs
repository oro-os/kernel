//! TODO explain the boot protocol
//! TODO - memory layout
//! TODO - unmapping all memory in lower half
//! TODO   - all frames mapped with flag bit 9 (BIT_9) will also have the
//! TODO     physical frames reclaimed (leave 0 for modules and other non-reclaimable
//! TODO     memory)
//! TODO - UNSTABLE ABI until later date
#![no_std]

use core::cmp::min;
pub use oro_ser2mem::{Allocator, Fake, Proxy, Serialize};
use oro_ser2mem::{CloneIterator, Ser2Mem};

/// Magic number used to verify a proper boot structure
pub const BOOT_MAGIC: u64 = u64::from_be_bytes(*b"ORO_BOOT");

/// MUST NOT be 511! MUST correspond to the beginning of the private kernel stack space.
/// MUST NOT BE IN LOWER HALF (<= 255)!
pub const RECURSIVE_PAGE_TABLE_INDEX: u16 = 256;
/// Oro sysapi page table index; the sysapi root structures
/// should be mapped at the beginning of this index's address space.
///
/// Any and all memory in this region will be reclaimed upon the kernel
/// initializing.
pub const ORO_SYSAPI_PAGE_TABLE_INDEX: u16 = 1;
/// Kernel stack page table index
///
/// Boot stages MUST make the last page in this memory region
/// non-present at all times, allocating a sufficient
/// stack space for the kernel to operate (growing downward),
/// and then keep all other pages lower than the first (lowest) stack
/// page as non-present.
///
/// Boot stages SHOULD allocate at least 16KiB for the kernel as a minimum,
/// and 64KiB if memory constraints are a non-issue.
pub const KERNEL_STACK_PAGE_TABLE_INDEX: u16 = 257;
/// Oro boot protocol index; all boot protocol structures
/// MUST be placed here, with the root structure located at
/// offset 0x0. If necessary, the kernel will free ALL MEMORY
/// in this index upon booting; do NOT place any other information
/// in this section!
pub const ORO_BOOT_PAGE_TABLE_INDEX: u16 = 258;
/// All secret heap allocations can be safely put here; inclusive.
///
/// Boot stages MUST NOT populate any memory in this region. The kernel
/// expects this region is completely empty.
pub const KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES: (u16, u16) = (259, 383);
/// All public heap allocations can be safely put here; inclusive.
///
/// Boot stages MUST NOT populate any memory in this region. The kernel
/// expects this region is completely empty.
pub const KERNEL_PUBLIC_HEAP_PAGE_TABLE_INDICES: (u16, u16) = (384, 447);
/// All userspace allocations can be safely put here; inclusive.
///
/// Any and all memory in this region will be reclaimed upon the kernel
/// initializing. Boot stages SHOULD utilize this region for any temporary
/// trampolines, memory maps, etc.
pub const USER_PAGE_TABLE_INDICES: (u16, u16) = (2, 255);

#[derive(Ser2Mem, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum MemoryRegionKind {
	Usable,
	Modules,
	Reserved,
}

#[derive(Ser2Mem)]
#[repr(C, align(8))]
pub struct MemoryRegion {
	pub base: u64,
	pub length: u64,
	pub kind: MemoryRegionKind,
}

#[derive(Ser2Mem)]
#[repr(C, align(4096))]
pub struct BootConfig<M>
where
	M: CloneIterator<Item = MemoryRegion>,
{
	/// Set to `BOOT_MAGIC`
	pub magic: u64,
	/// Set to a non-deterministic value, such as the current timestamp
	pub nonce: u64,
	/// Assign the result of `magic XOR nonce`
	pub nonce_xor_magic: u64,
	/// The list of memory regions made available to the machine.
	///
	/// It is IMPERATIVE that the order here matches the order used by
	/// the boot stage allocator, and that the boot stage allocator allocates
	/// frames IN ORDER from the beginning of the first usable region.
	pub memory_map: M,
}

impl<M> BootConfig<M>
where
	M: CloneIterator<Item = MemoryRegion>,
{
	/// Fast forward the memory maps passed to the Oro kernel
	/// based on the number of allocations performed at boot time.
	///
	/// # Safety
	///
	/// MUST ONLY be called after a successful call to serialize(),
	/// and MUST ONLY be called ONCE.
	pub unsafe fn fast_forward_memory_map<F>(
		&self,
		base_addr: u64,
		alloc_count: u64,
		is_relevant_page: F,
	) where
		F: Fn(&MemoryRegionKind) -> bool,
	{
		let boot_config = unsafe { &mut *(base_addr as *mut Proxy![Self]) };

		let mut remaining = alloc_count;

		// We do something here that is normally very, very unsafe.
		// However given the unsafe commentary here, it can't really
		// end up as UB as far as I'm aware. If this looks nasty and wrong
		// to you, it probably is. Please PR if you know of a better way.
		//
		// Given that the safety rules are adhered to, we can assume that
		// there is only ever one mutable reference. Further, we know that
		// the backing memory is indeed writable (since we implemented
		// the backing memory ourselves).
		//
		// Please, please never do this in normal Rust code. This is not good
		// Rust code. This is incredibly dangerous Rust code.
		let memory_map: &'static mut [Proxy![MemoryRegion]] = ::core::slice::from_raw_parts_mut(
			boot_config.memory_map.as_ptr() as u64 as *mut Proxy![MemoryRegion],
			boot_config.memory_map.len(),
		);

		for region in memory_map {
			if !is_relevant_page(&region.kind) {
				continue;
			}
			let region_total_pages = region.length >> 12;
			let pages_to_subtract = min(region_total_pages, remaining);
			remaining -= pages_to_subtract;

			let total_allocated_bytes = pages_to_subtract << 12;
			region.length -= total_allocated_bytes;
			region.base += total_allocated_bytes;

			if remaining == 0 {
				return;
			}
		}

		if remaining > 0 {
			panic!("still had allocations to perform after exhausting all memory regions");
		}
	}
}

#[inline(always)]
const fn sign_extend_48(addr: u64) -> u64 {
	addr | (((addr >> 47) & 1) * 0xFFFF_0000_0000_0000)
}

#[inline(always)]
pub const fn l4_to_range_48(idx: u16) -> (u64, u64) {
	let base = sign_extend_48(((idx as u64) & 511) << (12 + 9 + 9 + 9));
	(base, base | 0x7F_FFFF_FFFF)
}

#[inline(always)]
pub const fn l4_mkvirtaddr(idx1: u16, idx2: u16, idx3: u16, idx4: u16) -> u64 {
	sign_extend_48(
		((idx1 as u64) << (9 * 3 + 12))
			| ((idx2 as u64) << (9 * 2 + 12))
			| ((idx3 as u64) << (9 + 12))
			| ((idx4 as u64) << 12),
	)
}

#[inline(always)]
pub const fn l4_to_recursive_table(idx: u16) -> u64 {
	l4_mkvirtaddr(idx, idx, idx, idx)
}
