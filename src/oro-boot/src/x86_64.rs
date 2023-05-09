use oro_ser2mem::Ser2Mem;

/// MUST NOT be 511! MUST correspond to the beginning of the private kernel stack space
pub const RECURSIVE_PAGE_TABLE_INDEX: u16 = 256;
/// Oro sysapi page table index; the sysapi root structures
/// should be mapped at the beginning of this index's address space.
pub const ORO_SYSAPI_PAGE_TABLE_INDEX: u16 = 1;
/// Kernel stack page table index
///
/// Bootloaders should make the last page in this index
/// non-present at all times, allocating a sufficient
/// stack space for the kernel to operate (growing downward),
/// and then keep all other pages lower than the last stack
/// page as non-present.
pub const KERNEL_STACK_PAGE_TABLE_INDEX: u16 = 257;
/// Oro boot protocol index; all boot protocol structures
/// MUST be placed here, with the root structure located at
/// offset 0x0. If necessary, the kernel will free ALL MEMORY
/// in this index upon booting; do NOT place any other information
/// in this section!
pub const ORO_BOOT_PAGE_TABLE_INDEX: u16 = 258;
/// All secret heap allocations can be safely put here; inclusive.
pub const KERNEL_SECRET_HEAP_PAGE_TABLE_INDICES: (u16, u16) = (259, 383);
/// All public heap allocations can be safely put here; inclusive.
pub const KERNEL_PUBLIC_HEAP_PAGE_TABLE_INDICES: (u16, u16) = (384, 447);
/// All userspace allocations can be safely put here; inclusive.
pub const USER_PAGE_TABLE_INDICES: (u16, u16) = (2, 255);

#[derive(Ser2Mem, Copy, Clone, Debug)]
#[repr(u8)]
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
pub struct BootConfig {
	pub magic: u64,
	pub nonce: u64,
	pub nonce_xor_magic: u64,
	pub test_kind: MemoryRegionKind,
}

#[inline(always)]
fn sign_extend_48(addr: u64) -> u64 {
	addr | (((addr >> 47) & 1) * 0xFFFF_0000_0000_0000)
}

#[inline(always)]
pub fn l4_to_range_48(idx: u16) -> (u64, u64) {
	let base = sign_extend_48(((idx as u64) & 511) << (12 + 9 + 9 + 9));
	(base, base | 0x7F_FFFF_FFFF)
}
