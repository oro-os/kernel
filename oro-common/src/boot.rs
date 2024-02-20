use oro_ser2mem::{CloneIterator, Fake, Proxy, Ser2Mem};

/// The Oro boot protocol main configuration structure.
///
/// This structure is passed to the kernel via the bootloader,
/// where it is placed in a well-known location in memory
/// prior to jumping to _start().
///
/// For more information, see the documentation for the
/// [`oro-ser2mem`] and [`oro-bootloader-common`] crates.
#[derive(Ser2Mem)]
#[repr(C)]
pub struct BootConfig<M>
where
	M: CloneIterator<Item = MemoryRegion>,
{
	/// The number of instances that are being booted.
	/// Note that this _may not_ match the number of CPUs
	/// in the system.
	pub num_instances: u32,
	/// The list of memory regions made available to the machine.
	///
	/// Note that boot information (this struct) will not be written
	/// to regions marked as [`MemoryRegionType::Boot`].
	pub memory_regions: M,
}

/// The boot config type that results from a serialization.
/// Should only be used by the kernel.
pub type KernelBootConfig = Proxy![BootConfig<Fake<MemoryRegion>>];

/// Defines which instance of the CPU is being initialized.
/// On single-core systems, for example, this is always `Primary`.
///
/// Bootloaders must take care only to pass `Primary` to one
/// instance of whatever is running in an SMP environment.
#[derive(Ser2Mem, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum BootInstanceType {
	/// The primary CPU instance; performs initialization
	/// of all shared resources.
	Primary = 0,
	/// A secondary CPU instance; performs initialization
	/// of only its own resources.
	Secondary = 1,
}

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
}

/// A memory region.
///
/// Memory regions only refer to "main memory" regions,
/// i.e. regions of memory that are to be used for
/// generic read/write storage. They do not refer to
/// memory-mapped I/O regions, ACPI regions, video buffers, etc.
#[derive(Ser2Mem)]
#[repr(C)]
pub struct MemoryRegion {
	/// The base address of the memory range.
	pub base: u64,
	/// The length of the memory range.
	pub length: u64,
	/// The type of the memory region.
	pub ty: MemoryRegionType,
}

impl MemoryRegion {
	/// Gets the last address in the range (inclusive).
	#[inline]
	pub fn last(&self) -> u64 {
		self.base + self.length - 1
	}

	/// Gets the end address of the range (exclusive).
	#[inline]
	pub fn end(&self) -> u64 {
		self.base + self.length
	}

	/// Gets a new range that is aligned to the given size,
	/// both in base and length. If the base is unaligned,
	/// the base is rounded up to the next multiple of `align`.
	/// If the length is unaligned, the length is rounded
	/// down to the previous multiple of `align` after adjusting
	/// for the new base.
	#[cold]
	pub fn aligned(&self, align: u64) -> MemoryRegion {
		let base = (self.base + (align - 1)) & !(align - 1);
		let length = self.length - (base - self.base);
		let length = length & !(align - 1);
		MemoryRegion {
			base,
			length,
			ty: self.ty,
		}
	}
}
