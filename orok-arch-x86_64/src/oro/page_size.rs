//! Supplies the page size types for x86_64.

/// The available page sizes on x86_64.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[expect(
	clippy::arbitrary_source_item_ordering,
	reason = "ordered from smallest to largest page size"
)]
pub enum PageSize {
	/// 4 KiB page size.
	Size4KiB,
	/// 2 MiB page size.
	Size2MiB,
	/// 1 GiB page size.
	Size1GiB,
}

impl orok_arch_base::PageSize for PageSize {
	#[inline]
	fn page_size_bytes(&self) -> usize {
		match *self {
			Self::Size4KiB => 4 * 1024,
			Self::Size2MiB => 2 * 1024 * 1024,
			Self::Size1GiB => 1024 * 1024 * 1024,
		}
	}
}
