use oro_ser2mem::Ser2Mem;

/// Defines the type of a memory region.
#[derive(Ser2Mem, Clone, Copy)]
#[repr(u8)]
#[non_exhaustive]
pub enum MemoryRegionType {
	/// The region is available for use by the kernel.
	Usable = 0,
	/// The region is usable, but should only be used
	/// after fully transitioning execution to the kernel.
	Boot = 1,
	/// The region is not available for use.
	Unusable = 2,
	/// Bad memory; the region is not available for use,
	/// as the memory is potentially faulty. Not all
	/// bootloaders will provide this information.
	Bad = 3,
}

/// An extension trait for [`MemoryRegion`] and its proxy.
pub trait MemoryRegion: Sized {
	/// Gets the base address.
	fn base(&self) -> u64;

	/// Gets the length of the region.
	fn length(&self) -> u64;

	/// Gets the type of the region.
	fn ty(&self) -> MemoryRegionType;

	/// Creates a new region given a base, length and type
	fn new(base: u64, length: u64, ty: MemoryRegionType) -> Self;

	/// Gets the last address in the range (inclusive).
	#[inline]
	fn last(&self) -> u64 {
		self.base() + self.length() - 1
	}

	/// Gets the end address of the range (exclusive).
	#[inline]
	fn end(&self) -> u64 {
		self.base() + self.length()
	}

	/// Gets a new range that is aligned to the given size,
	/// both in base and length. If the base is unaligned,
	/// the base is rounded up to the next multiple of `align`.
	/// If the length is unaligned, the length is rounded
	/// down to the previous multiple of `align` after adjusting
	/// for the new base.
	#[cold]
	fn aligned(&self, align: u64) -> Self {
		let base = (self.base() + (align - 1)) & !(align - 1);
		let length = self.length() - (base - self.base());
		let length = length & !(align - 1);
		Self::new(base, length, self.ty())
	}
}
