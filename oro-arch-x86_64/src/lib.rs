//! x86_64 architecture support crate for the
//! [Oro Operating System](https://github.com/oro-os/kernel)
//! kernel.
//!
//! # Safety
//! To support x86_64 when implementing a preboot stage, please read
//! **both** `oro_boot::boot_to_kernel`'s documentation as well the
//! following safety requirements **carefully**.
//!
//! ## Memory
//! There are a few memory requirements that the x86_64 architecture support
//! mandates:
//!
//! ### Direct Maps
//! The Oro x86_64 architecture assumes a direct map of all physical memory
//! is direct mapped into the the address space. The implementation of a
//! [`oro_common::mem::translate::PhysicalAddressTranslator`] is required
//! to map physical addresses to virtual addresses in a deterministic fashion.
//!
//! While the memory regions do not technically need to be offset-based, it's
//! highly recommended to do so for ease of implementation. The common library
//! provides an [`oro_common::mem::translate::OffsetPhysicalAddressTranslator`]
//! that can be used if a simple offset needs to be applied to the physical
//! address to form a virtual address.
//!
//! ### Higher-Half Mapping
//! The Oro x86_64 architecture assumes the lower half of the
//! address space is "free game" during the preboot (bootloader)
//! stage. This is required for the execution handoff to the kernel,
//! whereby some common stubs are mapped both into the target kernel
//! address space as well as the current address space such that
//! the execution can resume after page tables are switched out.
//!
//! If possible, higher-half direct maps are advised. If not possible,
//! attempt to direct-map in the lower quarter of the address space
//! to avoid conflicts with the stub mappings. Stubs are mapped into
//! L4/L5 index 255, but this is NOT a stable guarantee.
//!
//! ### Shared Page Tables
//! The Oro x86_64 architecture expects that all SMP cores invoking
//! `oro_common::boot_to_kernel` use a shared page table - that is,
//! the `cr3` register points to the same base address for all cores.
//!
//! If for some reason `boot_to_kernel` is not used (and you're writing
//! your own transfer code), each secondary must have a shallow-cloned
//! CR3 (whereby the L4 page itself is uniquely allocated for each core,
//! but has duplicate content across all cores, thus pointing to the same
//! L3/2/1 physical pages).
//!
//! ### After-Transfer Behavior
//! All memory mapped in the lower half is reclaimed by the page frame
//! allocator after the transfer to the kernel. Any boot-time allocations
//! that are used only for the transfer to the kernel by the preboot
//! environment can be placed there for automatic cleanup by the kernel
//! once it has booted.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![allow(
	internal_features,
	clippy::verbose_bit_mask,
	clippy::module_name_repetitions
)]
#![feature(naked_functions, core_intrinsics, asm_const)]
#![cfg(not(all(doc, not(target_arch = "x86_64"))))]

#[cfg(debug_assertions)]
pub(crate) mod dbgutil;

pub(crate) mod acpi;
pub(crate) mod arch;
pub(crate) mod asm;
pub(crate) mod gdt;
pub(crate) mod interrupt;
pub(crate) mod mem;
pub(crate) mod reg;
pub(crate) mod xfer;

pub use self::arch::{init_kernel_primary, init_kernel_secondary, init_preboot_primary, X86_64};
