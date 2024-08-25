//! # Oro Boot Protocol
//! The Oro kernel boot protocol is a standardized interface for
//! booting into the Oro kernel.
//!
//! This crate provides all necessary types and documentation
//! for booting into the oro kernel from any environment, and
//! provides C headers for doing so from languages other than
//! Rust.
//!
//! The Oro boot protocol is heavily inspired by the Limine protocol
//! in that it uses versioned, tagged structures that are scanned for
//! and populated in the kernel address space after it is mapped.
//!
//! This crate documents the exact means by which the kernel's
//! protocol tags should be searched for and used.
//!
//! Users who wish to use a higher-level API to boot into the Oro
//! kernel should use the `oro-boot` crate, which provides a
//! safe and standardized interface for booting into the Oro kernel
//! without the need to implement the boot protocol directly.
//!
//! # Overview of Tag System
//! The boot protocol is based on request-response model, whereby
//! the kernel exports requests aligned on a 16-bit boundary somewhere
//! in the kernel address space. The bootloader is expected to scan
//! for these requests and populate them with the necessary data.
//!
//! Note that tags are architecture-endian, meaning the kernel
//! compiled for a little-endian system will have its tag bytes
//! reversed when compared to a kernel compiled for a big-endian
//! system.
//!
//! All discovered tags are expected to be populated, except for
//! those that are explicitly marked as optional.
//!
//! If a bootloader fails to populate a tag, the kernel is allowed
//! to panic if it cannot continue without it.
//!
//! # Discovering Requests
//! The Oro kernel ships as an ELF executable with a number of
//! program headers. One of the headers that is present is a
//! read-only header with the OS-specific flag `(1 << 21)`.
//!
//! In addition, all Kernel program headers will have the Oro
//! Kernel bit raised - `(1 << 20)`. This is to help prevent
//! bad user configuration from attempting to load a normal
//! ELF executable as a kernel.
//!
//! Once the segment is located, the bootloader is expected to
//! scan, on 16-byte boundaries, for tags (see the individual
//! requests' `TAG` constant, e.g. [`KernelSettingsRequest::TAG`]).
//!
//! The base address of the found tag is in turn the base address
//! of the [`RequestHeader`] structure, which is guaranteed to be
//! present at the beginning of every request.
//!
//! The request header is then used to determine the revision of
//! the request, and the appropriate data structure to populate.
//! If the bootloader does not recognize the revision, it should
//! skip the tag.
//!
//! The data directly after the request header is the data structure
//! to populate.
//!
//! # Populating Requests
//! The bootloader is expected to populate the request with the
//! appropriate data. The kernel will then use this data to
//! configure itself.
//!
//! The data should be mapped into the kernel's memory as specified
//! by the program header, with read-only permissions (thus the bootloader
//! must edit the memory prior to setting up the kernel's page tables).
//!
//! Upon populating a request, its `populated` value must be set to
//! `0xFF`. Bootloaders _should_ first make sure that the value was
//! `0x00` before populating the request, as a sanity check that
//! some bug or corruption did not occur.
#![cfg_attr(not(test), no_std)]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(clippy::too_many_lines)] // Seems to be a bug in clippy with the macro expansion

#[cfg(all(feature = "utils", oro_build_protocol_header))]
compile_error!("The `utils` feature cannot be enabled when building the boot protocol C header.");

mod macros;
#[cfg(feature = "utils")]
pub mod util;

/// The type of the kernel request tag.
pub type Tag = u64;

macros::oro_boot_protocol! {
	/// Main settings for the kernel.
	b"ORO_KRNL" => KernelSettings {
		0 => {
			/// The virtual offset of the linear map of physical memory.
			pub linear_map_offset: u64,
		}
	}

	/// A request for the memory map.
	b"ORO_MMAP" => MemoryMap {
		0 => {
			/// The number of entries in the memory map.
			pub entry_count: u64,
			/// The physical address of the first [`MemoryMapEntrye`] in the list.
			/// Must be aligned to the same alignment as the `MemoryMapEntry` structure.
			///
			/// If there are no entries, this value is ignored by the kernel - however,
			/// note that the kernel will expect at least one entry (otherwise it cannot
			/// initialize and will be effectively useless).
			pub entries: u64,
		}
	}

	/// Kernel request for the Advanced Configuration and
	/// Power Interface (ACPI) Root System Description
	/// Pointer (RSDP).
	///
	/// On x86, this is mandatory.
	///
	/// On other architectures that support other
	/// forms of configuration (e.g. DeviceTree, PSCI),
	/// this is optional.
	///
	/// If a device tree blob or PSCI configuration is used
	/// it takes precedence over the RSDP.
	b"ORO_ACPI" => Acpi {
		0 => {
			/// The physical address of the RSDP.
			pub rsdp: u64,
		}
	}

	/// **THIS IS TEMPORARY AND WILL BE REMOVED.**
	///
	/// Temporary request for the PFA head. This is to be removed
	/// after the kernel boot sequence is refactored.
	b"ORO_PFAH" => PfaHead {
		0 => {
			/// The physical address of the PFA head.
			pub pfa_head: u64,
		}
	}
}

/// A memory map entry, representing a chunk of physical memory
/// available to the system.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MemoryMapEntry {
	/// The base address of the memory region.
	pub base:   u64,
	/// The length of the memory region.
	pub length: u64,
	/// The type of the memory region.
	pub ty:     MemoryMapEntryType,
	/// The physical address of the next entry in the list,
	/// or `0` if this is the last entry.
	pub next:   u64,
}

impl PartialOrd for MemoryMapEntry {
	fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
		self.base.partial_cmp(&other.base)
	}
}

impl PartialEq for MemoryMapEntry {
	fn eq(&self, other: &Self) -> bool {
		self.base == other.base
	}
}

/// The type of a memory map entry.
///
/// For any unknown types, the bootloader should specify
/// [`MemoryMapEntryType::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum MemoryMapEntryType {
	/// Memory that is either unusable or reserved, or some type
	/// of memory that is available to the system but not any
	/// specific type usable by the kernel.
	Unknown           = 0,
	/// General memory immediately usable by the kernel.
	Usable            = 1,
	/// Memory that is used by the bootloader but that can be
	/// reclaimed by the kernel.
	BootloaderReclaim = 2,
	/// Memory that holds either the kernel itself, root ring modules,
	/// or other boot-time binary data (e.g. `DeviceTree` blobs).
	///
	/// This memory is not reclaimed nor written to by the kernel.
	Modules           = 3,
	/// Bad memory. This memory is functionally equivalent to
	/// `Unknown`, but is used to denote memory that is known to
	/// be bad, broken, or malfunctioning. It is reported to the user
	/// as such.
	Bad               = 4,
}
