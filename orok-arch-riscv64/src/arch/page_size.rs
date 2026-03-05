/// Page size types for RISC-V 64-bit architecture.

/// Page sizes for the RISC-V 64-bit architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum PageSize {
	/// 4 KiB page size.
	Size4K   = 4096,
	/// 2 MiB page size.
	Size2M   = 2 * 1024 * 1024,
	/// 1 GiB page size (gigapage).
	Size1G   = 1024 * 1024 * 1024,
	/// 512 GiB page size (terapage).
	Size512G = 512 * 1024 * 1024 * 1024,
	/// 256 TiB page size (petapage).
	Size256T = 256 * 1024 * 1024 * 1024 * 1024,
}

impl PageSize {
	/// Returns the size of the page in bytes.
	pub fn size_bytes(&self) -> usize {
		*self as usize
	}
}
