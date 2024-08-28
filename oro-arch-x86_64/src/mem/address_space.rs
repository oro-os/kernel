//! Provides the implementation of the [`AddressSpace`] trait for the x86_64 architecture.
//!
//! This code describes the overall address space layout used by the kernel and userspace processes.

use super::{paging::PageTable, paging_level::PagingLevel, segment::MapperHandle};
use crate::{
	asm::cr3,
	mem::{paging::PageTableEntry, segment::AddressSegment},
};
use oro_common::mem::{
	mapper::AddressSpace, pfa::alloc::PageFrameAllocate, translate::PhysicalAddressTranslator,
};

/// A handle to an address space for the x86_64 architecture.
///
/// Address spaces are created for each process, as well as once for the kernel.
pub struct AddressSpaceHandle {
	/// The base physical address of the L4 table for this address space.
	/// This is the value set to the CR3 register.
	pub base_phys:    u64,
	/// The paging level of this address space. This is simply cached
	/// to avoid repeated register lookups.
	pub paging_level: PagingLevel,
}

impl MapperHandle for AddressSpaceHandle {
	fn base_phys(&self) -> u64 {
		self.base_phys
	}

	fn paging_level(&self) -> PagingLevel {
		self.paging_level
	}
}

/// The main layout description for the x86_64 architecture.
///
/// This struct describes not only the page table indices for each
/// logical kernel / userspace memory segment, but also the flags
/// used for each segment.
pub struct AddressSpaceLayout;

#[rustfmt::skip]
impl AddressSpaceLayout {
	/// The index for kernel transfer stubs.
	/// NOTE(qix-): All lower-half indices are automatically reclaimed
	/// NOTE(qix-): by the kernel after boot.
	pub const STUBS_IDX: usize = 255;
	/// The stack space range
	pub const KERNEL_STACK_IDX: usize = 257;
	/// The index for where the GDT lives.
	pub const GDT_IDX: usize = 258;
	/// The direct map range
	pub const DIRECT_MAP_IDX: (usize, usize) = (259, 300);
	/// The index for the kernel core-local segment.
	pub const KERNEL_CORE_LOCAL_IDX: usize = 350;
	/// The segment for the ring registry
	pub const KERNEL_RING_REGISTRY_IDX: usize = 400;
	/// The segment for the module instance registry
	pub const KERNEL_MODULE_INSTANCE_REGISTRY_IDX: usize = 401;
	/// The segment for the port registry
	pub const KERNEL_PORT_REGISTRY_IDX: usize = 402;
	/// The kernel executable range, shared by the RX, RO, and RW segments.
	pub const KERNEL_EXE_IDX: usize = 511;
}

impl AddressSpaceLayout {
	/// Returns an internal descriptor used to map the kernel transfer stubs.
	#[inline(always)]
	pub const fn stubs() -> <Self as AddressSpace>::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (AddressSpaceLayout::STUBS_IDX, AddressSpaceLayout::STUBS_IDX),
			entry_template: PageTableEntry::new().with_present().with_writable(),
			intermediate_entry_template: PageTableEntry::new().with_present().with_writable(),
		};

		&DESCRIPTOR
	}

	/// Returns an internal descriptor used to map the kernel stack.
	#[inline(always)]
	pub const fn kernel_stack() -> <Self as AddressSpace>::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_STACK_IDX,
				AddressSpaceLayout::KERNEL_STACK_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec(),
		};

		&DESCRIPTOR
	}

	/// Returns an internal descriptor used to map the GDT.
	#[inline(always)]
	pub const fn gdt() -> <Self as AddressSpace>::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (AddressSpaceLayout::GDT_IDX, AddressSpaceLayout::GDT_IDX),
			entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_global(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_global(),
		};

		&DESCRIPTOR
	}
}

/// Intermediate page table entry template for the kernel code segment.
///
/// Defined here so that the overlapping kernel segments can share the same
/// intermediate entry, differences between which would cause indeterministic
/// behavior.
const KERNEL_EXE_INTERMEDIATE_ENTRY: PageTableEntry = PageTableEntry::new()
	.with_user()
	.with_global()
	.with_present();

// TODO(qix-): When const trait methods are stabilized, mark these as const.
unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static AddressSegment;

	unsafe fn current_supervisor_space<P>(_translator: &P) -> Self::SupervisorHandle
	where
		P: PhysicalAddressTranslator,
	{
		Self::SupervisorHandle {
			base_phys:    cr3(),
			paging_level: PagingLevel::current_from_cpu(),
		}
	}

	fn new_supervisor_space<A, P>(alloc: &mut A, translator: &P) -> Option<Self::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			(*(translator.to_virtual_addr(base_phys) as *mut PageTable)).reset();
		}

		Some(Self::SupervisorHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn duplicate_supervisor_space_shallow<A, P>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
		translator: &P,
	) -> Option<Self::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			(*(translator.to_virtual_addr(base_phys) as *mut PageTable)).shallow_copy_from(
				&*(translator.to_virtual_addr(space.base_phys) as *const PageTable),
			);
		}

		Some(Self::SupervisorHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn kernel_code() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_EXE_IDX,
				AddressSpaceLayout::KERNEL_EXE_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_user()
				.with_global()
				.with_present(),
			intermediate_entry_template: KERNEL_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn kernel_data() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_EXE_IDX,
				AddressSpaceLayout::KERNEL_EXE_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: KERNEL_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn kernel_rodata() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_EXE_IDX,
				AddressSpaceLayout::KERNEL_EXE_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec(),
			intermediate_entry_template: KERNEL_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn kernel_ring_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_RING_REGISTRY_IDX,
				AddressSpaceLayout::KERNEL_RING_REGISTRY_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}

	fn kernel_port_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_PORT_REGISTRY_IDX,
				AddressSpaceLayout::KERNEL_PORT_REGISTRY_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}

	fn kernel_module_instance_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_MODULE_INSTANCE_REGISTRY_IDX,
				AddressSpaceLayout::KERNEL_MODULE_INSTANCE_REGISTRY_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}

	fn kernel_core_local() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_CORE_LOCAL_IDX,
				AddressSpaceLayout::KERNEL_CORE_LOCAL_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}

	fn direct_map() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::DIRECT_MAP_IDX,
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable()
				.with_write_through(),
			intermediate_entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}
}
