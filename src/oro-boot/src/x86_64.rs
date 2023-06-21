//! TODO explain the boot protocol
//! TODO - memory layout
//! TODO - unmapping all memory in lower half

use oro_ser2mem::{CloneIterator, Ser2Mem};

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
	/// Set to `oro_boot::BOOT_MAGIC`
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
