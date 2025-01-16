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
#![expect(clippy::too_many_lines)] // Seems to be a bug in clippy with the macro expansion
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

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
			pub length: u32,
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

	/// Kernel request for the video buffer.
	///
	/// Optional, but if provided, the kernel will initialize
	/// a `ROOT_BOOT_VBUF_V0` interface on the root ring,
	/// allowing for it to be mapped into a module's address
	/// space.
	b"ORO_VBUF" => VideoBuffers {
		0 => {
			/// The physical address of the first [`RGBVideoBuffer`] in the list.
			/// Must be aligned to the same alignment as the `VideoBuffer` structure.
			///
			/// If there are no video buffers, this value must be zero.
			pub next: u64,
		}
	}
}

/// A module to load into the kernel on the root ring.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Module {
	/// The highest 64 bits of the module 128 bit ID.
	///
	/// The module ID **must not** be reserved, or
	/// the kernel will reject loading it.
	///
	/// See the `oro-id` crate for more information.
	pub id_high: u64,
	/// The lowest 64 bits of the module 128 bit ID.
	///
	/// The module ID **must not** be reserved, or
	/// the kernel will reject loading it.
	///
	/// See the `oro-id` crate for more information.
	/// The physical base address of the module.
	pub id_low:  u64,
	/// The physical start address of the module.
	pub base:    u64,
	/// The length of the module.
	pub length:  u64,
	/// The physical address of the next module in the list,
	/// or `0` if this is the last module.
	pub next:    u64,
}

#[cfg(feature = "utils")]
impl crate::macros::Sealed for Module {}

#[cfg(feature = "utils")]
impl crate::util::SetNext for Module {
	fn set_next(&mut self, next: u64) {
		self.next = next;
	}
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
	Unknown     = 0,
	/// General memory immediately usable by the kernel.
	Usable      = 1,
	/// Memory that holds either the kernel itself, root ring modules,
	/// or other boot-time binary data (e.g. `DeviceTree` blobs).
	///
	/// This memory is not reclaimed nor written to by the kernel.
	Modules     = 2,
	/// Bad memory. This memory is functionally equivalent to
	/// `Unknown`, but is used to denote memory that is known to
	/// be bad, broken, or malfunctioning. It is reported to the user
	/// as such.
	Bad         = 3,
	/// Boot protocol reclaimable memory
	///
	/// Memory that is used to populate the kernel's boot protocol
	/// response structures can be reclaimed after the kernel boots.
	/// Any memory that is allocated in order to populate the kernel's
	/// boot protocol response structures should be marked as `Reclaimable`.
	Reclaimable = 4,
	/// Memory that belongs to the frame buffer, if any.
	FrameBuffer = 5,
}

/// A video buffer to map into the kernel's address space.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RGBVideoBuffer {
	/// The physical address of the video buffer.
	///
	/// **Addresses must be page-aligned or they are skipped by the kernel.**
	pub base:           u64,
	/// The width of the buffer.
	pub width:          u64,
	/// The height of the buffer.
	pub height:         u64,
	/// The row pitch of the buffer. Not always a clean multiple
	/// of the width by any particular value; users should assume
	/// padding bytes might exist.
	pub row_pitch:      u64,
	/// The number of bits per pixel.
	pub bits_per_pixel: u16,
	/// The red mask. Applied using `v & ((1 << red_mask) - 1)`.
	pub red_mask:       u8,
	/// The red shift. Applied using `v << red_shift`.
	pub red_shift:      u8,
	/// The green mask. Applied using `v & ((1 << green_mask) - 1)`.
	pub green_mask:     u8,
	/// The green shift. Applied using `v << green_shift`.
	pub green_shift:    u8,
	/// The blue mask. Applied using `v & ((1 << blue_mask) - 1)`.
	pub blue_mask:      u8,
	/// The blue shift. Applied using `v << blue_shift`.
	pub blue_shift:     u8,
	/// The physical address of the next video buffer in the list,
	/// or `0` if this is the last module.
	pub next:           u64,
}

#[cfg(feature = "utils")]
impl crate::macros::Sealed for RGBVideoBuffer {}

#[cfg(feature = "utils")]
impl crate::util::SetNext for RGBVideoBuffer {
	fn set_next(&mut self, next: u64) {
		self.next = next;
	}
}
