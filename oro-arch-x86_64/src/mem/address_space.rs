//! Provides the implementation of the [`AddressSpace`] trait for the x86_64 architecture.
//!
//! This code describes the overall address space layout used by the kernel and userspace processes.

use oro_mem::{
	mapper::AddressSpace,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use super::{paging::PageTable, paging_level::PagingLevel, segment::MapperHandle};
use crate::{
	asm::cr3,
	mem::{paging::PageTableEntry, segment::AddressSegment},
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
	fn base_phys(&self) -> Phys {
		unsafe { Phys::from_address_unchecked(self.base_phys) }
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

// NOTE(qix-): Please keep this sorted.
#[rustfmt::skip]
impl AddressSpaceLayout {
	/// The index for the secondary core boot stubs.
	/// Only used during boot; do not change. Can overlap
	/// if the segment is used for userspace.
	pub const KERNEL_SECONDARY_BOOT_IDX: usize = 0;

	/// The index for the module segments.
	pub const MODULE_EXE_IDX: (usize, usize) = (5, 16);
	/// The index for the module thread stack segment.
	pub const MODULE_THREAD_STACK_IDX: usize = 17;
	/// The index for the module thread interrupt stack.
	pub const MODULE_INTERRUPT_STACK_IDX: usize = 18;

	/// The recursive index for the page table.
	pub const RECURSIVE_IDX: usize = 256;
	/// The stack space range
	pub const KERNEL_STACK_IDX: usize = 257;
	/// The direct map range
	pub const LINEAR_MAP_IDX: (usize, usize) = (259, 320);
	/// The index for the kernel core-local segment.
	pub const KERNEL_CORE_LOCAL_IDX: usize = 350;

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

/// Intermediate page table entry template for the module code/data segments.
///
/// Defined here so that the overlapping module segments can share the same
/// intermediate entry, differences between which would cause indeterministic
/// behavior.
const MODULE_EXE_INTERMEDIATE_ENTRY: PageTableEntry = PageTableEntry::new()
	.with_user()
	.with_present()
	.with_writable();

impl AddressSpaceLayout {
	/// Adds the recursive mapping to the provided page table.
	pub fn map_recursive_entry(handle: &AddressSpaceHandle) {
		// SAFETY(qix-): We can reasonably assuming that the `AddressSpaceHandle`
		// SAFETY(qix-): is valid if it's been constructed by us.
		unsafe {
			Phys::from_address_unchecked(handle.base_phys).as_mut_unchecked::<PageTable>()
				[Self::RECURSIVE_IDX] = PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec()
				.with_global()
				.with_address(handle.base_phys);
		}
	}

	/// Returns the linear map segment for the supervisor space.
	#[must_use]
	pub fn linear_map() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::LINEAR_MAP_IDX,
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

	/// Shallow-duplicates the current address space into a new one
	/// at the given physical address.
	pub fn copy_shallow_into(handle: &AddressSpaceHandle, into_phys: u64) {
		unsafe {
			Phys::from_address_unchecked(into_phys)
				.as_mut_unchecked::<PageTable>()
				.shallow_copy_from(
					Phys::from_address_unchecked(handle.base_phys).as_ref_unchecked::<PageTable>(),
				);
		}
	}

	/// Returns a segment for the secondary core boot stub.
	#[must_use]
	pub fn secondary_boot_stub_code() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_SECONDARY_BOOT_IDX,
				AddressSpaceLayout::KERNEL_SECONDARY_BOOT_IDX,
			),
			entry_template: PageTableEntry::new().with_present(),
			intermediate_entry_template: PageTableEntry::new().with_present().with_writable(),
		};

		&DESCRIPTOR
	}

	/// Returns a segment for the secondary core boot stub's stack
	/// mapping.
	#[must_use]
	pub fn secondary_boot_stub_stack() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_SECONDARY_BOOT_IDX,
				AddressSpaceLayout::KERNEL_SECONDARY_BOOT_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec(),
			intermediate_entry_template: PageTableEntry::new().with_present().with_writable(),
		};

		&DESCRIPTOR
	}

	/// Returns a segment for the module code segment.
	#[must_use]
	pub fn module_code() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new().with_user().with_present(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	/// Returns a segment for the module data segment.
	#[must_use]
	pub fn module_data() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new()
				.with_present()
				.with_no_exec()
				.with_writable(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	/// Returns a segment for the module read-only data segment.
	#[must_use]
	pub fn module_rodata() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new().with_present().with_no_exec(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	/// Returns a segment for the module's interrupt thread stack.
	///
	/// This MUST NOT overlap with any other segment, must be
	/// writable, and must NOT be user-accessible (despite being
	/// in the user address space). It must also not be executable.
	#[must_use]
	pub fn module_interrupt_stack() -> &'static AddressSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::MODULE_INTERRUPT_STACK_IDX,
				AddressSpaceLayout::MODULE_INTERRUPT_STACK_IDX,
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
}

#[expect(clippy::missing_docs_in_private_items)]
macro_rules! registries {
	($($name:ident => $idx:ident),* $(,)?) => {
		$(fn $name() -> Self::SupervisorSegment {
			const DESCRIPTOR: AddressSegment = AddressSegment {
				valid_range: (
					AddressSpaceLayout::$idx,
					AddressSpaceLayout::$idx,
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
		})*
	}
}

/// Intermediate page table entry template for the kernel code segment.
///
/// Defined here so that the overlapping kernel segments can share the same
/// intermediate entry, differences between which would cause indeterministic
/// behavior.
const KERNEL_EXE_INTERMEDIATE_ENTRY: PageTableEntry = PageTableEntry::new()
	.with_global()
	.with_present()
	.with_writable();

// TODO(qix-): When const trait methods are stabilized, mark these as const.
unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static AddressSegment;
	type UserHandle = AddressSpaceHandle;
	type UserSegment = &'static AddressSegment;

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

	unsafe fn current_supervisor_space() -> Self::SupervisorHandle {
		Self::SupervisorHandle {
			base_phys:    cr3(),
			paging_level: PagingLevel::current_from_cpu(),
		}
	}

	fn new_supervisor_space<A>(alloc: &mut A) -> Option<Self::SupervisorHandle>
	where
		A: Alloc,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			Phys::from_address_unchecked(base_phys)
				.as_mut_unchecked::<PageTable>()
				.reset();
		}

		Some(Self::SupervisorHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn new_user_space<A>(space: &Self::SupervisorHandle, alloc: &mut A) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		let duplicated = Self::duplicate_supervisor_space_shallow(space, alloc)?;

		// Unmap core-local segments.
		for segment in [Self::kernel_core_local(), Self::kernel_stack()] {
			// SAFETY(qix-): We're purposefully not reclaiming the memory here.
			unsafe {
				segment.unmap_without_reclaim(&duplicated);
			}
		}

		// Supervisor and userspace handles are the same on x86_64.
		Some(duplicated)
	}

	fn duplicate_supervisor_space_shallow<A: Alloc>(
		space: &Self::SupervisorHandle,
		alloc: &mut A,
	) -> Option<Self::SupervisorHandle> {
		let base_phys = alloc.allocate()?;

		unsafe {
			Phys::from_address_unchecked(base_phys)
				.as_mut_unchecked::<PageTable>()
				.shallow_copy_from(
					Phys::from_address_unchecked(space.base_phys).as_ref_unchecked::<PageTable>(),
				);
		}

		Some(Self::SupervisorHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn duplicate_user_space_shallow<A>(
		space: &Self::UserHandle,
		alloc: &mut A,
	) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		// Supervisor and userspace handles are the same on x86_64.
		Self::duplicate_supervisor_space_shallow(space, alloc)
	}

	fn free_user_space<A>(space: Self::UserHandle, alloc: &mut A)
	where
		A: Alloc,
	{
		// SAFETY(qix-): We can guarantee that if we have a valid handle,
		// SAFETY(qix-): we own this physical page and can free it.
		unsafe { alloc.free(space.base_phys) };
	}

	fn module_thread_stack() -> Self::UserSegment {
		#[expect(clippy::missing_docs_in_private_items)]
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::MODULE_THREAD_STACK_IDX,
				AddressSpaceLayout::MODULE_THREAD_STACK_IDX,
			),
			entry_template: PageTableEntry::new()
				.with_user()
				.with_writable()
				.with_no_exec()
				.with_present(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn kernel_code() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
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
		#[expect(clippy::missing_docs_in_private_items)]
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
		#[expect(clippy::missing_docs_in_private_items)]
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

	fn kernel_stack() -> <Self as AddressSpace>::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
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

	fn kernel_core_local() -> Self::SupervisorSegment {
		#[expect(clippy::missing_docs_in_private_items)]
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
}
