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
//! requests' `TAG` constant, e.g. [`MemoryMapRequest::TAG`]).
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
//!
//! # C Header Generation
//! This crate supports generating a C header file that can be used
//! to boot into the Oro kernel from C or other languages.
//!
//! To generate the header file, you can run the following command
//! (or equivalent for your platform):
//!
//! ```sh
//! env ORO_BUILD_PROTOCOL_HEADER=1 cargo build -p oro-boot-protocol
//! ```
//!
//! The resulting header file is emitted to `$CARGO_TARGET_DIR/oro-boot.h`.
#![cfg_attr(not(test), no_std)]
#![allow(clippy::too_many_lines)] // Seems to be a bug in clippy with the macro expansion

// NOTE(qix-): This module is quite hairy with macros, both procedural and otherwise.
// NOTE(qix-): If you're trying to make sense of the types in here, it's probably
// NOTE(qix-): best to generate the documentation via `cargo doc --open` and refer to
// NOTE(qix-): that, as it will very nicely lay out all of the types and their
// NOTE(qix-): relationships - especially since rust-analyzer has an especially hard time
// NOTE(qix-): with this module.
// NOTE(qix-):
// NOTE(qix-): Alternatively, you can generate the C header file (emits to
// NOTE(qix-): `$CARGO_TARGET_DIR/oro-boot.h`) and read the types / docs from there.
// NOTE(qix-): See the crate comments for more information on how to do that.

#[cfg(all(feature = "utils", oro_build_protocol_header))]
compile_error!("The `utils` feature cannot be enabled when building the boot protocol C header.");

mod macros;
#[cfg(feature = "utils")]
pub mod util;

/// The type of the kernel request tag.
pub type Tag = u64;

macros::oro_boot_protocol! {
	/// A request for the memory map.
	b"ORO_MMAP" => MemoryMap {
		0 => {
			/// The physical address of the first [`MemoryMapEntrye`] in the list.
			/// Must be aligned to the same alignment as the `MemoryMapEntry` structure.
			///
			/// If there are no entries, this value must be zero - however,
			/// note that the kernel will expect at least one entry (otherwise it cannot
			/// initialize and will be effectively useless).
			pub next: u64,
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

	/// Kernel request for the DeviceTree blob, if available.
	///
	/// If no DeviceTree blob is provided on architectures that
	/// support it, some kernels may opt to use the ACPI configuration
	/// instead (via the [`AcpiRequest`]).
	///
	/// Otherwise, if no DeviceTree blob is provided and no ACPI
	/// configuration is provided, the kernel may panic.
	b"ORO_DTRB" => DeviceTree {
		0 => {
			/// The physical address of the DeviceTree blob.
			pub base: u64,
			/// The length of the DeviceTree blob.
			pub length: u64,
		}
	}

	/// Kernel request for a list of modules to load and
	/// place onto the root ring.
	b"ORO_MODS" => Modules {
		0 => {
			/// The physical address of the first [`Module`] in the list.
			/// Must be aligned to the same alignment as the `Module` structure.
			///
			/// If there are no modules, this value must be zero - however,
			/// note that the kernel will not do anything particularly useful
			/// without additional application-specific modules.
			///
			/// # ⚠️ Security Advisory ⚠️
			///
			/// These modules have **full, unrestricted access to the system**
			/// They are not sandboxed in any way. **Untrusted modules should not
			/// be loaded** unless you know what you're doing.
			pub next: u64,
		}
	}
}

/// A module to load into the kernel on the root ring.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Module {
	/// The physical base address of the module.
	pub base:   u64,
	/// The length of the module.
	pub length: u64,
	/// The physical address of the next module in the list,
	/// or `0` if this is the last module.
	pub next:   u64,
}

/// A memory map entry, representing a chunk of physical memory
/// available to the system.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryMapEntry {
	/// The base address of the memory region.
	///
	/// This is _not_ guaranteed to be aligned to any particular
	/// value; the kernel will handle alignment internally.
	pub base:   u64,
	/// The length of the memory region.
	///
	/// This is _not_ guaranteed to be aligned to any particular
	/// value; the kernel will handle alignment internally.
	pub length: u64,
	/// The type of the memory region.
	pub ty:     MemoryMapEntryType,
	/// How much of this memory region, in bytes, was
	/// used by the bootloader. That memory will be
	/// reclaimed by the kernel after the bootloader
	/// requests have been processed.
	///
	/// This field is ignored for regions not marked
	/// as [`MemoryMapEntryType::Usable`], and should
	/// be 0 for those regions.
	///
	/// Note that this is _not_ guaranteed to be aligned
	/// to any particular value; the kernel will handle
	/// alignment internally.
	///
	/// # x86 / x86_64 Specific
	/// On x86 / x86_64, the first 1MiB of memory is
	/// reserved and **must not** be used by the bootloader.
	///
	/// All bytes that fall under this region, regardless of
	/// their type, should be added to the `used` field's
	/// count.
	pub used:   u64,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u16)]
pub enum MemoryMapEntryType {
	/// Memory that is either unusable or reserved, or some type
	/// of memory that is available to the system but not any
	/// specific type usable by the kernel.
	#[default]
	Unknown = 0,
	/// General memory immediately usable by the kernel.
	Usable  = 1,
	/// Memory that holds either the kernel itself, root ring modules,
	/// or other boot-time binary data (e.g. `DeviceTree` blobs).
	///
	/// This memory is not reclaimed nor written to by the kernel.
	Modules = 2,
	/// Bad memory. This memory is functionally equivalent to
	/// `Unknown`, but is used to denote memory that is known to
	/// be bad, broken, or malfunctioning. It is reported to the user
	/// as such.
	Bad     = 3,
}
