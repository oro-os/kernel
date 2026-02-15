//! Supplies the page size types for AArch64.

pub use crate::arch::GranuleSize as PageSize;

impl orok_arch_base::PageSize for PageSize {
	fn page_size_bytes(&self) -> usize {
		self.size_bytes()
	}
}
