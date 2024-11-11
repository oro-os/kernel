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
#![cfg(not(all(doc, not(target_arch = "x86_64"))))]
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

pub mod asm;
pub mod boot;
pub mod gdt;
pub mod handler;
pub mod interrupt;
pub mod lapic;
pub mod mem;
pub mod reg;
pub mod task;
pub mod tss;

pub(crate) mod init;

use core::{cell::UnsafeCell, mem::MaybeUninit};

use mem::address_space::AddressSpaceLayout;
use oro_elf::{ElfClass, ElfEndianness, ElfMachine};
use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, MapError, UnmapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

/// The ELF class of the x86_64 architecture.
pub const ELF_CLASS: ElfClass = ElfClass::Class64;
/// The ELF endianness of the x86_64 architecture.
pub const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
/// The ELF machine of the x86_64 architecture.
pub const ELF_MACHINE: ElfMachine = ElfMachine::X86_64;

/// Zero-sized type for specifying the architecture-specific types
/// used throughout the `oro-kernel` crate.
pub(crate) struct Arch;

impl oro_kernel::Arch for Arch {
	type AddrSpace = crate::mem::address_space::AddressSpaceLayout;
	type CoreState = CoreState;
	type ThreadState = ThreadState;

	fn initialize_thread_mappings(
		thread: &<Self::AddrSpace as oro_mem::mapper::AddressSpace>::UserHandle,
		thread_state: &mut Self::ThreadState,
	) -> Result<(), oro_mem::mapper::MapError> {
		// Map only a page, with a stack guard.
		// Must match below, in `ThreadState::default`.
		let irq_stack_segment = AddressSpaceLayout::interrupt_stack();
		let stack_high_guard = irq_stack_segment.range().1 & !0xFFF;
		let stack_start = stack_high_guard - 0x1000;
		#[cfg(debug_assertions)]
		let stack_low_guard = stack_start - 0x1000;

		debug_assert_eq!(thread_state.irq_stack_ptr, stack_high_guard as u64);

		// Make sure the guard pages are unmapped.
		// More of a debug check, as this should never be the case
		// with a bug-free implementation.
		#[cfg(debug_assertions)]
		{
			match irq_stack_segment.unmap(thread, stack_high_guard) {
				Ok(phys) => panic!("interrupt stack high guard was already mapped at {phys:016X}"),
				Err(UnmapError::NotMapped) => {}
				Err(err) => {
					panic!("interrupt stack high guard encountered error when unmapping: {err:?}")
				}
			}

			match irq_stack_segment.unmap(thread, stack_low_guard) {
				Ok(phys) => panic!("interrupt stack low guard was already mapped at {phys:016X}"),
				Err(UnmapError::NotMapped) => {}
				Err(err) => {
					panic!("interrupt stack low guard encountered error when unmapping: {err:?}")
				}
			}
		}

		// Map the stack page.
		let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
		irq_stack_segment.map(thread, stack_start, phys)?;

		// Now write the initial `iretq` information to the frame.
		// SAFETY(qix-): We know that these are valid addresses.
		unsafe {
			let page_slice = core::slice::from_raw_parts_mut(
				Phys::from_address_unchecked(phys).as_mut_ptr_unchecked(),
				4096 >> 3,
			);
			let written =
				crate::task::initialize_user_irq_stack(page_slice, thread_state.entry_point);
			thread_state.irq_stack_ptr -= written;
		}

		Ok(())
	}

	fn reclaim_thread_mappings(
		thread: &<Self::AddrSpace as oro_mem::mapper::AddressSpace>::UserHandle,
		_thread_state: &mut Self::ThreadState,
	) -> Result<(), UnmapError> {
		// SAFETY(qix-): The module interrupt stack space is fully reclaimable and never shared.
		unsafe { AddressSpaceLayout::interrupt_stack().unmap_all_and_reclaim(thread) }
	}
}

/// Type alias for the Oro kernel core-local instance type.
pub(crate) type Kernel = oro_kernel::Kernel<Arch>;

/// The guaranteed offset of the task state segment (TSS) in the GDT.
///
/// Verified at boot time, such that this index can be used without having
/// to perform a lookup.
pub const TSS_GDT_OFFSET: u16 = 0x28;

/// Architecture-specific core-local state.
pub(crate) struct CoreState {
	/// The LAPIC (Local Advanced Programmable Interrupt Controller)
	/// for the core.
	pub lapic: lapic::Lapic,
	/// The core's local GDT
	///
	/// Only valid after the Kernel has been initialized
	/// and properly mapped.
	pub gdt: UnsafeCell<MaybeUninit<gdt::Gdt<7>>>,
	/// The TSS (Task State Segment) for the core.
	pub tss: UnsafeCell<tss::Tss>,
	/// The kernel's stored stack pointer.
	pub kernel_stack: UnsafeCell<u64>,
	/// The IRQ head of the kernel stack (with GP registers)
	pub kernel_irq_stack: UnsafeCell<u64>,
}

// XXX(qix-): This is temporary. The core state is not currently used
// XXX(qix-): across core boundaries, so we can manually mark it as `Sync`.
// XXX(qix-): I want to fix up how the kernel guards this value so as to drop
// XXX(qix-): the `Sync` requirement altogether, but for now this is sufficient,
// XXX(qix-): if not a little fragile.
unsafe impl Sync for CoreState {}

/// x86_64-specific thread state.
pub(crate) struct ThreadState {
	/// The thread's interrupt stack pointer.
	pub irq_stack_ptr: u64,
	/// The thread's entry point.
	pub entry_point:   u64,
}

impl ThreadState {
	/// Creates a new thread state with the given entry point.
	pub fn new(entry: u64) -> Self {
		// Must match above in `Arch::initialize_thread_mappings`.
		let irq_stack_segment = AddressSpaceLayout::interrupt_stack();
		let stack_high_guard = irq_stack_segment.range().1 & !0xFFF;

		Self {
			irq_stack_ptr: stack_high_guard as u64,
			entry_point:   entry,
		}
	}
}
