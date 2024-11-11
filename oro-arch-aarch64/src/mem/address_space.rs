//! Implements the Oro-specific address space layout for the Aarch64 architecture.

use oro_mem::{
	mapper::AddressSpace,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

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

/// A lightweight handle to a TTBR1 address space.
pub struct Ttbr1Handle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys: Phys,
}

/// A lightweight handle to a TTBR0 address space.
pub struct Ttbr0Handle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys: Phys,
}

/// Defines differentiating information and functions for each of the
/// address space handles.
pub trait TtbrHandle {
	/// All virtual addresses mapped/unmapped have this value subtracted from
	/// them before being passed to the page table walker.
	// NOTE(qix-): We currently specify that the kernel uses `TCR_EL1.TnSZ=16`,
	// NOTE(qix-): so we hard-code this value here (as opposed to `current_supervisor_space`).
	// NOTE(qix-): Unlike `current_supervisor_space`, this function will probably have to be
	// NOTE(qix-): updated in the future if other `TnSZ` values are supported or used.
	const VIRT_START: usize;

	/// Returns the base physical address of the root of the page tables
	/// associated with this address space.
	#[must_use]
	fn base_phys(&self) -> Phys;
}

impl TtbrHandle for Ttbr1Handle {
	const VIRT_START: usize = 0xFFFF_0000_0000_0000;

	fn base_phys(&self) -> Phys {
		self.base_phys
	}
}

impl TtbrHandle for Ttbr0Handle {
	const VIRT_START: usize = 0;

	fn base_phys(&self) -> Phys {
		self.base_phys
	}
}

/// The Oro-specific address space layout implementation for the Aarch64 architecture.
pub struct AddressSpaceLayout;

// NOTE(qix-): Please keep this sorted.
#[rustfmt::skip]
impl AddressSpaceLayout {
	/// The index for kernel transfer stubs.
	/// Since we identity map the stubs, we must specify an index
	/// range that spans the entirety of the lower half.
	pub const STUBS_IDX: (usize, usize) = (0, 255);

	/// The index for the module segments.
	pub const MODULE_EXE_IDX: (usize, usize) = (5, 16);

	/// The recursive entry indices.
	pub const RECURSIVE_ENTRY_IDX: (usize, usize) = (256, 259);
	/// The stack space range
	pub const KERNEL_STACK_IDX: usize = 260;
	/// The linear map range
	pub const LINEAR_MAP_IDX: (usize, usize) = (261, 300);
	/// Reserved area for boot / on the fly mappings.
	///
	/// There is no associated descriptor for this index;
	/// it's used however needed for the boot process.
	pub const BOOT_RESERVED_IDX: usize = 350;
	/// The segment for the kernel core-local data.
	pub const KERNEL_CORE_LOCAL_IDX: usize = 375;

	/// The kernel executable range, shared by the RX, RO, and RW segments.
	///
	/// MUST BE 511.
	pub const KERNEL_EXE_IDX: usize = 511;
}

impl AddressSpaceLayout {
	/// Installs the recursive page table entry.
	pub fn map_recursive_entry(mapper: &mut Ttbr1Handle) {
		unsafe {
			let pt = mapper.base_phys.as_mut_unchecked::<PageTable>();

			pt[Self::RECURSIVE_ENTRY_IDX.0] = L0PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys.address_u64())
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 1] = L1PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys.address_u64())
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 2] = L2PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys.address_u64())
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 3] = L3PageTableBlockDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys.address_u64())
				.with_block_access_permissions(PageTableEntryBlockAccessPerm::KernelRWUserNoAccess)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.with_not_secure()
				.with_mair_index(u64::from(MairEntry::DirectMap.index()))
				.into();

			debug_assert_eq!(
				(Self::RECURSIVE_ENTRY_IDX.0 + 3),
				Self::RECURSIVE_ENTRY_IDX.1
			);
		}
	}

	/// Returns the segment descriptor for the kernel transfer stubs.
	#[must_use]
	pub fn stubs() -> <Self as AddressSpace>::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
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
	#[must_use]
	pub fn kernel_stack() -> <Self as AddressSpace>::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
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

	/// Creates a new supervisor (EL1) address space that addresses
	/// the TT0 address range (i.e. for use with `TTBR0_EL1`). Uses
	/// the global allocator.
	///
	/// This probably isn't used by the kernel, but instead by the
	/// preboot environment to map stubs.
	pub fn new_supervisor_space_ttbr0() -> Option<Ttbr0Handle> {
		Self::new_supervisor_space_ttbr0_in(&mut oro_mem::global_alloc::GlobalPfa)
	}

	/// Creates a new supervisor (EL1) address space that addresses
	/// the TT0 address range (i.e. for use with `TTBR0_EL1`). Uses
	/// the given allocator.
	///
	/// This probably isn't used by the kernel, but instead by the
	/// preboot environment to map stubs.
	pub fn new_supervisor_space_ttbr0_in<A>(alloc: &mut A) -> Option<Ttbr0Handle>
	where
		A: Alloc,
	{
		// NOTE(qix-): This is just a temporary sanity check to make sure
		// NOTE(qix-): we aren't going to blow up later on if we change
		// NOTE(qix-): something about the address space settings.
		debug_assert_eq!(
			TcrEl1::load().tt0_range(),
			(0x0000_0000_0000_0000, 0x0000_FFFF_FFFF_FFFF)
		);

		let base_phys = alloc
			.allocate()
			.map(|addr| unsafe { Phys::from_address_unchecked(addr) })?;

		unsafe {
			base_phys.as_mut_unchecked::<PageTable>().reset();
		}

		Some(Ttbr0Handle { base_phys })
	}
}

/// L0 intermediate PTE for the kernel executable segment.
///
/// Defined here as a constant since it's used within overlapping
/// segments and any differences will cause indeterministic behavior.
const KERNEL_EXE_L0: L0PageTableDescriptor = unsafe {
	L0PageTableDescriptor::new()
		.with_valid()
		.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
		.with_user_no_exec()
};
/// L1 intermediate PTE for the kernel executable segment.
///
/// Defined here as a constant since it's used within overlapping
/// segments and any differences will cause indeterministic behavior.
const KERNEL_EXE_L1: L1PageTableDescriptor = unsafe {
	L1PageTableDescriptor::new()
		.with_valid()
		.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
		.with_user_no_exec()
};
/// L2 intermediate PTE for the kernel executable segment.
///
/// Defined here as a constant since it's used within overlapping
/// segments and any differences will cause indeterministic behavior.
const KERNEL_EXE_L2: L2PageTableDescriptor = unsafe {
	L2PageTableDescriptor::new()
		.with_valid()
		.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
		.with_user_no_exec()
};

unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = Ttbr1Handle;
	type SupervisorSegment = &'static Segment;
	type UserHandle = Ttbr0Handle;
	type UserSegment = &'static Segment;

	unsafe fn current_supervisor_space() -> Self::SupervisorHandle {
		Self::SupervisorHandle {
			base_phys: Phys::from_address_unchecked(crate::asm::load_ttbr1()),
		}
	}

	fn new_supervisor_space_in<A>(alloc: &mut A) -> Option<Self::SupervisorHandle>
	where
		A: Alloc,
	{
		// NOTE(qix-): This is just a temporary sanity check to make sure
		// NOTE(qix-): we aren't going to blow up later on if we change
		// NOTE(qix-): something about the address space settings.
		debug_assert_eq!(
			TcrEl1::load().tt1_range(),
			(0xFFFF_0000_0000_0000, 0xFFFF_FFFF_FFFF_FFFF)
		);

		let base_phys = alloc
			.allocate()
			.map(|addr| unsafe { Phys::from_address_unchecked(addr) })?;

		unsafe {
			base_phys.as_mut_unchecked::<PageTable>().reset();
		}

		Some(Ttbr1Handle { base_phys })
	}

	fn new_user_space_in<A>(
		_space: &Self::SupervisorHandle,
		alloc: &mut A,
	) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		let base_phys = alloc
			.allocate()
			.map(|addr| unsafe { Phys::from_address_unchecked(addr) })?;

		unsafe {
			base_phys.as_mut_unchecked::<PageTable>().reset();
		}

		Some(Ttbr0Handle { base_phys })
	}

	fn free_user_space_in<A>(_space: Self::UserHandle, _alloc: &mut A)
	where
		A: Alloc,
	{
		todo!();
	}

	fn duplicate_supervisor_space_shallow_in<A>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
	) -> Option<Self::SupervisorHandle>
	where
		A: Alloc,
	{
		let base_phys = alloc
			.allocate()
			.map(|addr| unsafe { Phys::from_address_unchecked(addr) })?;

		unsafe {
			let pt = base_phys.as_mut_unchecked::<PageTable>();
			pt.reset();
			pt.shallow_copy_from(space.base_phys.as_ref_unchecked());
		}

		Some(Self::SupervisorHandle { base_phys })
	}

	fn duplicate_user_space_shallow_in<A>(
		space: &Self::UserHandle,
		alloc: &mut A,
	) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		let base_phys = alloc
			.allocate()
			.map(|addr| unsafe { Phys::from_address_unchecked(addr) })?;

		unsafe {
			let pt = base_phys.as_mut_unchecked::<PageTable>();
			pt.reset();
			pt.shallow_copy_from(space.base_phys.as_ref_unchecked());
		}

		Some(Self::UserHandle { base_phys })
	}

	fn kernel_code() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       KERNEL_EXE_L0,
				l1_table_template: KERNEL_EXE_L1,
				l2_table_template: KERNEL_EXE_L2,
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelROUserNoAccess,
					)
					.with_user_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::NormalMemory.index() as u64),
			}
		};

		&DESCRIPTOR
	}

	fn kernel_data() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       KERNEL_EXE_L0,
				l1_table_template: KERNEL_EXE_L1,
				l2_table_template: KERNEL_EXE_L2,
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
		#[expect(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_EXE_IDX,
					AddressSpaceLayout::KERNEL_EXE_IDX,
				),
				l0_template:       KERNEL_EXE_L0,
				l1_table_template: KERNEL_EXE_L1,
				l2_table_template: KERNEL_EXE_L2,
				l3_template:       L3PageTableBlockDescriptor::new()
					.with_valid()
					.with_block_access_permissions(
						PageTableEntryBlockAccessPerm::KernelROUserNoAccess,
					)
					.with_user_no_exec()
					.with_kernel_no_exec()
					.with_not_secure()
					.with_mair_index(MairEntry::NormalMemory.index() as u64),
			}
		};

		&DESCRIPTOR
	}

	fn kernel_stack() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_STACK_IDX,
					AddressSpaceLayout::KERNEL_STACK_IDX,
				),
				l0_template:       KERNEL_EXE_L0,
				l1_table_template: KERNEL_EXE_L1,
				l2_table_template: KERNEL_EXE_L2,
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

	fn kernel_core_local() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_CORE_LOCAL_IDX,
					AddressSpaceLayout::KERNEL_CORE_LOCAL_IDX,
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

	fn user_thread_stack() -> Self::UserSegment {
		todo!();
	}
}
