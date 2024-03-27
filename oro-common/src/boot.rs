//! Boot protocol structures and definitions.
//!
//! This module contains the structures and definitions for the Oro boot protocol.
//! The boot protocol is a set of structures that are passed to the kernel by the
//! bootloader, and are used to configure the kernel and provide information about
//! the system's hardware and other resources.
#![allow(clippy::module_name_repetitions)]

use crate::mem::{MemoryRegion, MemoryRegionType};
use oro_ser2mem::{Fake, Proxy, Ser2Mem};

/// Useful for bootloaders to use type alias impl declarations
/// for iterator types.
pub use oro_ser2mem::CloneIterator;

/// The Oro boot protocol main configuration structure.
///
/// This structure is passed to the kernel via the bootloader,
/// where it is placed in a well-known location in memory
/// prior to jumping to `_start()`.
///
/// For more information, see the documentation for the
/// [`oro-ser2mem`] crate.
#[derive(Ser2Mem)]
#[repr(C)]
pub struct BootConfig<M>
where
	M: CloneIterator<Item = BootMemoryRegion>,
{
	/// The number of instances that are being booted.
	/// Note that this _may not_ match the number of CPUs
	/// in the system.
	pub num_instances: u64,
	/// The list of memory regions made available to the machine.
	pub memory_regions: M,
}

/// The boot config type that results from a serialization.
/// Should only be used by the kernel.
pub type KernelBootConfig = Proxy![BootConfig<Fake<BootMemoryRegion>>];

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

/// A boot config memory region.
///
/// Memory regions only refer to "main memory" regions,
/// i.e. regions of memory that are to be used for
/// generic read/write storage. They do not refer to
/// memory-mapped I/O regions, ACPI regions, video buffers, etc.
#[derive(Ser2Mem)]
#[repr(C)]
pub struct BootMemoryRegion {
	/// The base address of the memory range.
	pub base: u64,
	/// The length of the memory range.
	pub length: u64,
	/// The type of the memory region.
	pub ty: MemoryRegionType,
}

impl MemoryRegion for BootMemoryRegion {
	#[inline]
	fn base(&self) -> u64 {
		self.base
	}

	#[inline]
	fn length(&self) -> u64 {
		self.length
	}

	#[inline]
	fn ty(&self) -> MemoryRegionType {
		self.ty
	}

	#[inline]
	fn new(base: u64, length: u64, ty: MemoryRegionType) -> Self {
		Self { base, length, ty }
	}
}

impl MemoryRegion for Proxy![BootMemoryRegion] {
	#[inline]
	fn base(&self) -> u64 {
		self.base
	}

	#[inline]
	fn length(&self) -> u64 {
		self.length
	}

	#[inline]
	fn ty(&self) -> MemoryRegionType {
		self.ty
	}

	#[inline]
	fn new(base: u64, length: u64, ty: MemoryRegionType) -> Self {
		Self { base, length, ty }
	}
}
