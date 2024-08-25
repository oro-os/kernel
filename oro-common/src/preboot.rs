//! Types for configuring the boot-to-kernel routine. Used exclusively
//! by the preboot environment by way of the `oro_boot` crate.

use crate::mem::{region::MemoryRegion, translate::PhysicalAddressTranslator};

/// Provides the types used by the primary core configuration values
/// specified in [`PrebootConfig`].
pub trait PrebootPrimaryConfig {
	/// The type of memory region provided by the pre-boot environment.
	type MemoryRegion: MemoryRegion + Sized + 'static;

	/// The type of memory region iterator provided by the pre-boot environment.
	type MemoryRegionIterator: Iterator<Item = Self::MemoryRegion> + Clone + 'static;

	/// The type of physical-to-virtual address translator used by the pre-boot environment.
	type PhysicalAddressTranslator: PhysicalAddressTranslator + Clone + Sized + 'static;

	/// Whether or not "bad" memory regions are reported by the pre-boot environment.
	const BAD_MEMORY_REPORTED: bool;
}

/// Provides the initialization routine with configuration information for
/// each of the cores.
///
/// # Safety
/// See `oro_boot::boot_to_kernel()` for information regarding the safe use of this enum.
pub enum PrebootConfig<P>
where
	P: PrebootPrimaryConfig,
{
	/// The primary core configuration
	Primary {
		/// The **unique** core ID
		core_id: u64,
		/// The number of instances that are being booted
		num_instances: u64,
		/// An iterator over all memory regions available to the system
		memory_regions: P::MemoryRegionIterator,
		/// The physical-to-virtual address translator for the core
		physical_address_translator: P::PhysicalAddressTranslator,
		/// The module definition for the Oro kernel itself.
		kernel_module: ModuleDef,
		/// For systems that support ACPI, the physical address of the RSDP.
		/// Must be relative to the linear offset base.
		rsdp: Option<u64>,
	},
	/// A secondary core configuration
	Secondary {
		/// The **unique** core ID
		core_id: u64,
		/// The physical-to-virtual address translator for the core
		physical_address_translator: P::PhysicalAddressTranslator,
	},
}

impl<P> PrebootConfig<P>
where
	P: PrebootPrimaryConfig,
{
	/// Returns the core ID of the configuration.
	pub fn core_id(&self) -> u64 {
		match self {
			PrebootConfig::Primary { core_id, .. } | PrebootConfig::Secondary { core_id, .. } => {
				*core_id
			}
		}
	}

	/// Returns a reference to the physical-to-virtual address translator for the core.
	pub fn physical_address_translator(&self) -> &P::PhysicalAddressTranslator {
		match self {
			PrebootConfig::Primary {
				physical_address_translator,
				..
			}
			| PrebootConfig::Secondary {
				physical_address_translator,
				..
			} => physical_address_translator,
		}
	}
}

/// A module definition, providing base locations, lengths, and
/// per-module initialization configuration for both the kernel
/// and root-ring modules.
///
/// Modules must be ELF files (see the [`crate::elf`] module for
/// more information on what constitutes an ELF file valid for
/// the Oro operating system).
#[derive(Clone, Copy, Debug)]
pub struct ModuleDef {
	/// The base address of the module.
	/// **MUST** be available in the pre-boot address space.
	/// **MUST** be aligned to a 4-byte boundary.
	pub base:   usize,
	/// The length of the module in bytes.
	pub length: u64,
}
