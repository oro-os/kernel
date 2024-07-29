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
	reg::tcr_el1::TcrEl1,
};
use oro_common::mem::{AddressSpace, PageFrameAllocate, PhysicalAddressTranslator};

/// A lightweight handle to an address space.
pub struct AddressSpaceHandle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys:  u64,
	/// Lower bound of the address range covered by this address space.
	///
	/// All virtual addresses mapped/unmapped have this value subtracted from
	/// them before being passed to the page table walker.
	pub virt_start: usize,
}

/// The Oro-specific address space layout implementation for the Aarch64 architecture.
pub struct AddressSpaceLayout;

impl AddressSpaceLayout {
	/// The index for the kernel boot protocol.
	pub const BOOT_INFO_IDX: usize = 302;
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

	/// Returns a new supervisor address space handle with the given virtual start address.
	///
	/// Returns `None` if any allocation(s) fail.
	///
	/// # Safety
	/// The caller must ensure that the given virtual start address is valid.
	unsafe fn new_supervisor_space_with_start<A, P>(
		alloc: &mut A,
		translator: &P,
		virt_start: usize,
	) -> Option<AddressSpaceHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			(*(translator.to_virtual_addr(base_phys) as *mut PageTable)).reset();
		}

		Some(AddressSpaceHandle {
			base_phys,
			virt_start,
		})
	}

	/// Creates a new supervisor (EL1) address space that addresses
	/// the TT0 address range (i.e. for use with `TTBR0_EL1`).
	///
	/// This probably isn't used by the kernel, but instead by the
	/// preboot environment to map stubs.
	pub(crate) fn new_supervisor_space_tt0<A, P>(
		alloc: &mut A,
		translator: &P,
	) -> Option<<Self as AddressSpace>::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		unsafe { Self::new_supervisor_space_with_start(alloc, translator, 0) }
	}
}

unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static Segment;

	unsafe fn current_supervisor_space<P>(_translator: &P) -> Self::SupervisorHandle
	where
		P: PhysicalAddressTranslator,
	{
		// NOTE(qix-): Technically this isn't required since the kernel currently
		// NOTE(qix-): requires `TCR_EL1.TnSZ=16`, but it's cheap and not often
		// NOTE(qix-): called, so we'll just do it anyway.
		#[allow(clippy::cast_possible_truncation)]
		let (tt1_start, _) = TcrEl1::load().tt1_range();

		let base_phys = crate::asm::load_ttbr1();
		Self::SupervisorHandle {
			base_phys,
			virt_start: tt1_start,
		}
	}

	fn new_supervisor_space<A, P>(alloc: &mut A, translator: &P) -> Option<Self::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: PhysicalAddressTranslator,
	{
		// NOTE(qix-): We currently specify that the kernel uses `TCR_EL1.TnSZ=16`,
		// NOTE(qix-): so we hard-code this value here (as opposed to `current_supervisor_space`).
		// NOTE(qix-): Unlike `current_supervisor_space`, this function will probably have to be
		// NOTE(qix-): updated in the future if other `TnSZ` values are supported or used.
		unsafe { Self::new_supervisor_space_with_start(alloc, translator, 0xFFFF_0000_0000_0000) }
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

		Some(Self::SupervisorHandle {
			base_phys,
			virt_start: space.virt_start,
		})
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

	fn boot_info() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::BOOT_INFO_IDX,
					AddressSpaceLayout::BOOT_INFO_IDX,
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
}
