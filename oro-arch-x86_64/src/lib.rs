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
//! is direct mapped into the the address space. The direct map must be a
//! linear offset map.
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
#![expect(internal_features)]
#![cfg(any(doc, target_arch = "x86_64"))]
#![feature(naked_functions, core_intrinsics)]
// SAFETY(qix-): This is accepted, but moving slowly (in fact, probably the slowest
// SAFETY(qix-): I've seen - predates Rust 1.0). It simplifies the amount of `#[allow]`
// SAFETY(qix-): markers that have to be used when sign-extending addresses. The current
// SAFETY(qix-): plan to make it work on unambiguous (e.g. braced) statements looks like
// SAFETY(qix-): where it'll end up so I'm not too worried about using this feature flag.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/15701
#![feature(stmt_expr_attributes)]
// SAFETY(qix-): Required for GDT manipulation.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/76560
#![expect(incomplete_features)]
#![feature(generic_const_exprs)]
// SAFETY(qix-): Needed to make the system call key checks work inline.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/76001
#![feature(inline_const_pat)]
// SAFETY(qix-): Needed for clean code surrounding the default IDT table.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/83527
#![feature(macro_metavar_expr)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]
// TODO(qix-): This is a temporary measure; it'll be removed at some point.
#![expect(unsafe_op_in_unsafe_fn)]

pub mod asm;
pub mod boot;
pub mod core_local;
pub mod cpuid;
pub mod gdt;
pub mod iface;
pub mod instance;
pub mod interrupt;
pub mod lapic;
pub mod mem;
pub mod reg;
pub mod syscall;
pub mod task;
pub mod thread;
pub mod tss;

use oro_elf::{ElfClass, ElfEndianness, ElfMachine};
use oro_kernel::{iface::kernel::KernelInterface, table::Table};
use oro_mem::alloc::boxed::Box;

/// The ELF class of the x86_64 architecture.
pub const ELF_CLASS: ElfClass = ElfClass::Class64;
/// The ELF endianness of the x86_64 architecture.
pub const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
/// The ELF machine of the x86_64 architecture.
pub const ELF_MACHINE: ElfMachine = ElfMachine::X86_64;

/// Zero-sized type for specifying the architecture-specific types
/// used throughout the `oro-kernel` crate.
pub struct Arch;

impl oro_kernel::arch::Arch for Arch {
	type AddressSpace = crate::mem::address_space::AddressSpaceLayout;
	type CoreHandle = self::core_local::CoreHandle;
	type InstanceHandle = self::instance::InstanceHandle;
	type ThreadHandle = self::thread::ThreadHandle;

	fn fence() {
		// NOTE(qix-): This might be too strong for what we need.
		asm::strong_memory_fence();
	}

	fn register_kernel_interfaces(table: &mut Table<Box<dyn KernelInterface<Self>>>) {
		iface::register_kernel_interfaces(table);
	}
}

/// Type alias for the Oro kernel core-local instance type.
pub type Kernel = oro_kernel::Kernel<Arch>;
