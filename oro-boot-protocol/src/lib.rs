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

mod macros;

macros::oro_boot_protocol! {
	/// Main settings for the kernel.
	KernelSettings [b"ORO_KRNL"] {
		0 => {
			/// The total number of cores being booted.
			pub core_count: u64,
			/// The virtual offset of the linear map of physical memory.
			pub linear_map_offset: usize,
		}
	}
}
