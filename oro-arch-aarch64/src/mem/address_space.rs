//! Implements the Oro-specific address space layout for the Aarch64 architecture.

use crate::{
	mair::MairEntry,
	mem::{
		paging::{
			L0PageTableDescriptor, L1PageTableDescriptor, L2PageTableDescriptor,
			L3PageTableBlockDescriptor, PageTable, PageTableEntryBlockAccessPerm,
			PageTableEntryTableAccessPerm,
		},
		segment::Segment,
	},
};
use oro_common::mem::{AddressSpace, PageFrameAllocate, PhysicalAddressTranslator};

/// A lightweight handle to an address space.
pub struct AddressSpaceHandle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys: u64,
}

/// The Oro-specific address space layout implementation for the Aarch64 architecture.
pub struct AddressSpaceLayout;

impl AddressSpaceLayout {
	/// The direct map range
	pub const DIRECT_MAP_IDX: (usize, usize) = (258, 300);
	/// The kernel executable range, shared by the RX, RO, and RW segments.
	pub const KERNEL_EXE_IDX: usize = 511;
	/// The stack space range
	pub const KERNEL_STACK_IDX: usize = 257;
	/// The recursive index for the page table.
	pub const RECURSIVE_IDX: usize = 256;
	/// The index for kernel transfer stubs.
	/// Since we identity map the stubs, we must specify an index
	/// range that spans the entirety of the lower half.
	pub const STUBS_IDX: (usize, usize) = (0, 255);
}

impl AddressSpaceLayout {
	/// Returns the segment descriptor for the kernel transfer stubs.
	pub fn stubs() -> <Self as AddressSpace>::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       AddressSpaceLayout::STUBS_IDX,
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::NoEffect),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::NoEffect),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::NoEffect),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelROUserNoAccess,
					),
			}
		};

		&DESCRIPTOR
	}

	/// Returns the segment descriptor for the kernel stack.
	pub fn kernel_stack() -> <Self as AddressSpace>::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_STACK_IDX,
					AddressSpaceLayout::KERNEL_STACK_IDX,
				),
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_user_no_exec()
					.with_kernel_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::NormalMemory.index() as u64),
			}
		};

		&DESCRIPTOR
	}
}

unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static Segment;

	unsafe fn current_supervisor_space<P>(_translator: &P) -> Self::SupervisorHandle
	where
		P: PhysicalAddressTranslator,
	{
		let base_phys = crate::asm::load_ttbr1();
		Self::SupervisorHandle { base_phys }
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

		Some(Self::SupervisorHandle { base_phys })
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
			let pt = &mut *(translator.to_virtual_addr(base_phys) as *mut PageTable);
			pt.reset();
			pt.shallow_copy_from(
				&*(translator.to_virtual_addr(space.base_phys) as *const PageTable),
			);
		}

		Some(Self::SupervisorHandle { base_phys })
	}

	fn kernel_code() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec(),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec(),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec(),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelROUserNoAccess,
					)
					.with_user_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::KernelExe.index() as u64),
			}
		};

		&DESCRIPTOR
	}

	fn kernel_data() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_user_no_exec()
					.with_kernel_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::NormalMemory.index() as u64),
			}
		};

		&DESCRIPTOR
	}

	fn kernel_rodata() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelROUserNoAccess,
					)
					.with_user_no_exec()
					.with_kernel_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::KernelRo.index() as u64),
			}
		};

		&DESCRIPTOR
	}

	fn direct_map() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       AddressSpaceLayout::DIRECT_MAP_IDX,
				l0_template:       L0PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l1_table_template: L1PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l2_table_template: L2PageTableDescriptor::new()
					.with_valid()
					.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
					.with_user_no_exec()
					.with_kernel_no_exec(),
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelRWUserNoAccess,
					)
					.with_user_no_exec()
					.with_kernel_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::DirectMap.index() as u64),
			}
		};

		&DESCRIPTOR
	}
}
