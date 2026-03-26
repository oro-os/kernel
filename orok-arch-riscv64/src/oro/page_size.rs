//! Oro-specific RISC-V 64-bit page size types and implementations.
pub use crate::arch::PageSize;

impl orok_arch_base::PageSize for PageSize {
	fn page_size_bytes(&self) -> usize {
		self.size_bytes()
	}
}

impl orok_arch_base::ArchPageSize for super::Arch {
	type PageSize = PageSize;
}
