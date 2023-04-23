#![no_std]

pub mod x86_64 {
	/// All page table indices in this module are reserved for
	/// special functions of the OS.
	pub mod well_known {
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
	}
}
