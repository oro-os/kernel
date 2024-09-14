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
use oro_mem::{mapper::AddressSpace, pfa::alloc::Alloc, translate::Translator};

/// A lightweight handle to a TTBR1 address space.
pub struct Ttbr1Handle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys: u64,
}

/// A lightweight handle to a TTBR0 address space.
pub struct Ttbr0Handle {
	/// The base physical address of the root of the page tables
	/// associated with an address space.
	pub base_phys: u64,
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
	fn base_phys(&self) -> u64;
}

impl TtbrHandle for Ttbr1Handle {
	const VIRT_START: usize = 0xFFFF_0000_0000_0000;

	fn base_phys(&self) -> u64 {
		self.base_phys
	}
}

impl TtbrHandle for Ttbr0Handle {
	const VIRT_START: usize = 0;

	fn base_phys(&self) -> u64 {
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

	/// The segment for the ring registry
	pub const KERNEL_RING_REGISTRY_IDX: usize = 400;
	/// The segment for the ring item registry
	pub const KERNEL_RING_ITEM_REGISTRY_IDX: usize = 401;
	/// The segment for the ring list registry
	pub const KERNEL_RING_LIST_REGISTRY_IDX: usize = 402;

	/// The segment for the module instance registry
	pub const KERNEL_INSTANCE_REGISTRY_IDX: usize = 403;
	/// The segment for the module instance item registry
	pub const KERNEL_INSTANCE_ITEM_REGISTRY_IDX: usize = 404;
	/// The segment for the module instance list registry
	pub const KERNEL_INSTANCE_LIST_REGISTRY_IDX: usize = 405;

	/// The segment for the thread registry
	pub const KERNEL_THREAD_REGISTRY_IDX: usize = 406;
	/// The segment for the thread item registry
	pub const KERNEL_THREAD_ITEM_REGISTRY_IDX: usize = 407;
	/// The segment for the thread list registry
	pub const KERNEL_THREAD_LIST_REGISTRY_IDX: usize = 408;

	/// The segment for the module registry
	pub const KERNEL_MODULE_REGISTRY_IDX: usize = 409;
	/// The segment for the module item registry
	pub const KERNEL_MODULE_ITEM_REGISTRY_IDX: usize = 410;
	/// The segment for the module list registry
	pub const KERNEL_MODULE_LIST_REGISTRY_IDX: usize = 411;

	/// The segment for the port registry
	pub const KERNEL_PORT_REGISTRY_IDX: usize = 412;
	/// The segment for the port item registry
	pub const KERNEL_PORT_ITEM_REGISTRY_IDX: usize = 413;
	/// The segment for the port list registry
	pub const KERNEL_PORT_LIST_REGISTRY_IDX: usize = 414;

	/// The kernel executable range, shared by the RX, RO, and RW segments.
	///
	/// MUST BE 511.
	pub const KERNEL_EXE_IDX: usize = 511;
}

impl AddressSpaceLayout {
	/// Installs the recursive page table entry.
	pub fn map_recursive_entry(mapper: &mut Ttbr1Handle, pat: &impl Translator) {
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
	/// the TT0 address range (i.e. for use with `TTBR0_EL1`).
	///
	/// This probably isn't used by the kernel, but instead by the
	/// preboot environment to map stubs.
	pub fn new_supervisor_space_ttbr0<A, P>(alloc: &mut A, translator: &P) -> Option<Ttbr0Handle>
	where
		A: Alloc,
		P: Translator,
	{
		// NOTE(qix-): This is just a temporary sanity check to make sure
		// NOTE(qix-): we aren't going to blow up later on if we change
		// NOTE(qix-): something about the address space settings.
		debug_assert_eq!(
			TcrEl1::load().tt0_range(),
			(0x0000_0000_0000_0000, 0x0000_FFFF_FFFF_FFFF)
		);

		let base_phys = alloc.allocate()?;

		unsafe {
			(*translator.translate_mut::<PageTable>(base_phys)).reset();
		}

		Some(Ttbr0Handle { base_phys })
	}
}

#[expect(clippy::missing_docs_in_private_items)]
macro_rules! registries {
	($($name:ident => $idx:ident),* $(,)?) => {
		$(fn $name() -> Self::SupervisorSegment {
			static DESCRIPTOR: Segment = unsafe {
				Segment {
					valid_range:       (
						AddressSpaceLayout::$idx,
						AddressSpaceLayout::$idx,
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
		})*
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

	registries! {
		kernel_ring_registry => KERNEL_RING_REGISTRY_IDX,
		kernel_ring_item_registry => KERNEL_RING_ITEM_REGISTRY_IDX,
		kernel_ring_list_registry => KERNEL_RING_LIST_REGISTRY_IDX,
		kernel_instance_registry => KERNEL_INSTANCE_REGISTRY_IDX,
		kernel_instance_item_registry => KERNEL_INSTANCE_ITEM_REGISTRY_IDX,
		kernel_instance_list_registry => KERNEL_INSTANCE_LIST_REGISTRY_IDX,
		kernel_port_registry => KERNEL_PORT_REGISTRY_IDX,
		kernel_port_item_registry => KERNEL_PORT_ITEM_REGISTRY_IDX,
		kernel_port_list_registry => KERNEL_PORT_LIST_REGISTRY_IDX,
		kernel_thread_registry => KERNEL_THREAD_REGISTRY_IDX,
		kernel_thread_item_registry => KERNEL_THREAD_ITEM_REGISTRY_IDX,
		kernel_thread_list_registry => KERNEL_THREAD_LIST_REGISTRY_IDX,
		kernel_module_registry => KERNEL_MODULE_REGISTRY_IDX,
		kernel_module_item_registry => KERNEL_MODULE_ITEM_REGISTRY_IDX,
		kernel_module_list_registry => KERNEL_MODULE_LIST_REGISTRY_IDX,
	}

	unsafe fn current_supervisor_space<P>(_translator: &P) -> Self::SupervisorHandle
	where
		P: Translator,
	{
		Self::SupervisorHandle {
			base_phys: crate::asm::load_ttbr1(),
		}
	}

	fn new_supervisor_space<A, P>(alloc: &mut A, translator: &P) -> Option<Self::SupervisorHandle>
	where
		A: Alloc,
		P: Translator,
	{
		// NOTE(qix-): This is just a temporary sanity check to make sure
		// NOTE(qix-): we aren't going to blow up later on if we change
		// NOTE(qix-): something about the address space settings.
		debug_assert_eq!(
			TcrEl1::load().tt1_range(),
			(0xFFFF_0000_0000_0000, 0xFFFF_FFFF_FFFF_FFFF)
		);

		let base_phys = alloc.allocate()?;

		unsafe {
			(*translator.translate_mut::<PageTable>(base_phys)).reset();
		}

		Some(Ttbr1Handle { base_phys })
	}

	fn duplicate_supervisor_space_shallow<A, P>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
		translator: &P,
	) -> Option<Self::SupervisorHandle>
	where
		A: Alloc,
		P: Translator,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			let pt = &mut *translator.translate_mut::<PageTable>(base_phys);
			pt.reset();
			pt.shallow_copy_from(&*translator.translate(space.base_phys));
		}

		Some(Self::SupervisorHandle { base_phys })
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
}
