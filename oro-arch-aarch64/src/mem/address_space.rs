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
use oro_mem::{mapper::AddressSpace, pfa::alloc::PageFrameAllocate, translate::Translator};

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

// NOTE(qix-): Please keep this sorted.
#[rustfmt::skip]
impl AddressSpaceLayout {
	/// The index for kernel transfer stubs.
	/// Since we identity map the stubs, we must specify an index
	/// range that spans the entirety of the lower half.
	pub const STUBS_IDX: (usize, usize) = (0, 255);
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
	/// Installs the recursive page table entry.
	pub fn map_recursive_entry(mapper: &mut AddressSpaceHandle, pat: &impl Translator) {
		unsafe {
			let pt = &mut *pat.translate_mut::<PageTable>(mapper.base_phys);

			pt[Self::RECURSIVE_ENTRY_IDX.0] = L0PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys)
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 1] = L1PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys)
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 2] = L2PageTableDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys)
				.with_table_access_permissions(PageTableEntryTableAccessPerm::KernelOnly)
				.with_user_no_exec()
				.with_kernel_no_exec()
				.into();
			pt[Self::RECURSIVE_ENTRY_IDX.0 + 3] = L3PageTableBlockDescriptor::new()
				.with_valid()
				.with_address(mapper.base_phys)
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
	#[must_use]
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
		P: Translator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			(*translator.translate_mut::<PageTable>(base_phys)).reset();
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
	pub fn new_supervisor_space_tt0<A, P>(
		alloc: &mut A,
		translator: &P,
	) -> Option<<Self as AddressSpace>::SupervisorHandle>
	where
		A: PageFrameAllocate,
		P: Translator,
	{
		unsafe { Self::new_supervisor_space_with_start(alloc, translator, 0) }
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
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static Segment;

	unsafe fn current_supervisor_space<P>(_translator: &P) -> Self::SupervisorHandle
	where
		P: Translator,
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
		P: Translator,
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
		P: Translator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			let pt = &mut *translator.translate_mut::<PageTable>(base_phys);
			pt.reset();
			pt.shallow_copy_from(&*translator.translate(space.base_phys));
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
		#[allow(clippy::missing_docs_in_private_items)]
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
		#[allow(clippy::missing_docs_in_private_items)]
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
		#[allow(clippy::missing_docs_in_private_items)]
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

	fn kernel_ring_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_RING_REGISTRY_IDX,
					AddressSpaceLayout::KERNEL_RING_REGISTRY_IDX,
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

	fn kernel_module_instance_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_MODULE_INSTANCE_REGISTRY_IDX,
					AddressSpaceLayout::KERNEL_MODULE_INSTANCE_REGISTRY_IDX,
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

	fn kernel_port_registry() -> Self::SupervisorSegment {
		#[allow(clippy::missing_docs_in_private_items)]
		static DESCRIPTOR: Segment = unsafe {
			Segment {
				valid_range:       (
					AddressSpaceLayout::KERNEL_PORT_REGISTRY_IDX,
					AddressSpaceLayout::KERNEL_PORT_REGISTRY_IDX,
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
