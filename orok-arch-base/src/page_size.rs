//! Traits for defining architecture-specific page sizes.

/// Provides the architecture-specific page size types.
pub trait ArchPageSize {
	/// The page size type for the architecture.
	type PageSize: PageSize;
}

/// Trait representing page sizes available on the architecture.
pub trait PageSize: Sized + 'static {
	/// Returns the page size's size in bytes.
	///
	/// This is also treated as the alignment.
	fn page_size_bytes(&self) -> usize;
}
