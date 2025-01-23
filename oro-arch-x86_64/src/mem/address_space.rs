//! Provides the implementation of the [`AddressSpace`] trait for the x86_64 architecture.
//!
//! This code describes the overall address space layout used by the kernel and userspace processes.

use oro_mem::{
	mapper::{AddressSegment as _, AddressSpace},
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

	/// The index for the system ABI segment.
	///
	/// Must be placed in the lower half.
	pub const SYSABI: usize = 1;

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

	/// The index for the kernel syscall stack.
	/// This is a core-local stack that is used for handling syscalls.
	pub const KERNEL_SYSCALL_STACK_IDX: usize = 355;

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
			// NOTE(qix-): Reminder not to treat this as a leaf PTE. It's setting
			// NOTE(qix-): both intermediate entries as well as a leaf (technically,
			// NOTE(qix-): since that's how the CPU sees it). This had a global flag
			// NOTE(qix-): before, which really didn't need to be there to begin with,
			// NOTE(qix-): but it also made AMD choke on it. Please don't add it back.
			Phys::from_address_unchecked(handle.base_phys).as_mut_unchecked::<PageTable>()
				[Self::RECURSIVE_IDX] = PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec()
				.with_address(handle.base_phys);
		}
	}

	/// Returns the linear map segment for the supervisor space.
	#[must_use]
	pub fn linear_map() -> &'static AddressSegment {
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

	/// Returns the kernel syscall stack segment.
	#[must_use]
	pub fn kernel_syscall_stack() -> &'static AddressSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_SYSCALL_STACK_IDX,
				AddressSpaceLayout::KERNEL_SYSCALL_STACK_IDX,
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

	/// Returns a segment for the module's interrupt thread stack.
	///
	/// This MUST NOT overlap with any other segment, must be
	/// writable, and must NOT be user-accessible (despite being
	/// in the user address space). It must also not be executable.
	#[must_use]
	pub fn interrupt_stack() -> &'static AddressSegment {
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

/// Intermediate page table entry template for the kernel code segment.
///
/// Defined here so that the overlapping kernel segments can share the same
/// intermediate entry, differences between which would cause indeterministic
/// behavior.
const KERNEL_EXE_INTERMEDIATE_ENTRY: PageTableEntry =
	PageTableEntry::new().with_present().with_writable();

// TODO(qix-): When const trait methods are stabilized, mark these as const.
unsafe impl AddressSpace for AddressSpaceLayout {
	type SupervisorHandle = AddressSpaceHandle;
	type SupervisorSegment = &'static AddressSegment;
	type UserHandle = AddressSpaceHandle;
	type UserSegment = &'static AddressSegment;

	unsafe fn current_supervisor_space() -> Self::SupervisorHandle {
		Self::SupervisorHandle {
			base_phys:    cr3(),
			paging_level: PagingLevel::current_from_cpu(),
		}
	}

	fn new_supervisor_space_in<A>(alloc: &A) -> Option<Self::SupervisorHandle>
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

	fn new_user_space_in<A>(space: &Self::SupervisorHandle, alloc: &A) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		let duplicated = Self::duplicate_supervisor_space_shallow_in(space, alloc)?;

		// Unmap core-local segments.
		for segment in [Self::kernel_core_local(), Self::kernel_stack()] {
			// SAFETY(qix-): We're purposefully not reclaiming the memory here.
			unsafe {
				segment.unmap_all_without_reclaim(&duplicated);
			}
		}

		// Supervisor and userspace handles are the same on x86_64.
		Some(duplicated)
	}

	fn new_user_space_empty_in<A>(alloc: &A) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		let base_phys = alloc.allocate()?;

		unsafe {
			Phys::from_address_unchecked(base_phys)
				.as_mut_unchecked::<PageTable>()
				.reset();
		}

		Some(Self::UserHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn duplicate_supervisor_space_shallow_in<A: Alloc>(
		space: &Self::SupervisorHandle,
		alloc: &A,
	) -> Option<Self::SupervisorHandle> {
		let base_phys = alloc.allocate()?;

		unsafe {
			Phys::from_address_unchecked(base_phys)
				.as_mut_unchecked::<PageTable>()
				.shallow_copy_from(
					Phys::from_address_unchecked(space.base_phys)
						.as_ref::<PageTable>()
						.unwrap(),
				);
		}

		Some(Self::SupervisorHandle {
			base_phys,
			paging_level: PagingLevel::current_from_cpu(),
		})
	}

	fn duplicate_user_space_shallow_in<A>(
		space: &Self::UserHandle,
		alloc: &A,
	) -> Option<Self::UserHandle>
	where
		A: Alloc,
	{
		// Supervisor and userspace handles are the same on x86_64.
		Self::duplicate_supervisor_space_shallow_in(space, alloc)
	}

	fn free_user_space_handle_in<A>(space: Self::UserHandle, alloc: &A)
	where
		A: Alloc,
	{
		// SAFETY: Since user handles are not copyable (by contract, at least)
		// SAFETY: we can safely assume the page is owned by the handle and is
		// SAFETY: thus safe to free.
		unsafe {
			alloc.free(space.base_phys);
		}
	}

	fn free_user_space_deep_in<A>(space: Self::UserHandle, alloc: &A)
	where
		A: Alloc,
	{
		let pt = unsafe {
			Phys::from_address_unchecked(space.base_phys).as_mut_unchecked::<PageTable>()
		};

		// Iterate over all the pages in the page table and free them.
		match space.paging_level {
			PagingLevel::Level4 => {
				// NOTE(qix-): Only the lower half! The upper half is reserved for the kernel.
				for l0_idx in 0..=255 {
					let l0_entry = pt[l0_idx];
					if l0_entry.present() {
						// SAFETY: We can assume the address is valid if it's been placed in here.
						let l1 = unsafe {
							Phys::from_address_unchecked(l0_entry.address())
								.as_mut_unchecked::<PageTable>()
						};
						for l1_idx in 0..=511 {
							let l1_entry = l1[l1_idx];
							if l1_entry.present() {
								// SAFETY: We can assume the address is valid if it's been placed in here.
								let l2 = unsafe {
									Phys::from_address_unchecked(l1_entry.address())
										.as_mut_unchecked::<PageTable>()
								};
								for l2_idx in 0..=511 {
									let l2_entry = l2[l2_idx];
									if l2_entry.present() {
										// SAFETY: We can assume the address is valid if it's been placed in here.
										let l3 = unsafe {
											Phys::from_address_unchecked(l2_entry.address())
												.as_mut_unchecked::<PageTable>()
										};
										for l3_idx in 0..=511 {
											let l3_entry = l3[l3_idx];
											if l3_entry.present() {
												// SAFETY: We're sure this is a page we want to free.
												unsafe {
													alloc.free(l3_entry.address());
												}
											}
										}
										// SAFETY: We're sure this is a page we want to free.
										unsafe {
											alloc.free(l2_entry.address());
										}
									}
								}
								// SAFETY: We're sure this is a page we want to free.
								unsafe {
									alloc.free(l1_entry.address());
								}
							}
						}
						// SAFETY: We're sure this is a page we want to free.
						unsafe {
							alloc.free(l0_entry.address());
						}
					}
				}
			}
			PagingLevel::Level5 => {
				// NOTE(qix-): Only the lower half! The upper half is reserved for the kernel.
				for l0_idx in 0..=255 {
					let l0_entry = pt[l0_idx];
					if l0_entry.present() {
						// SAFETY: We can assume the address is valid if it's been placed in here.
						let l1 = unsafe {
							Phys::from_address_unchecked(l0_entry.address())
								.as_mut_unchecked::<PageTable>()
						};
						for l1_idx in 0..=511 {
							let l1_entry = l1[l1_idx];
							if l1_entry.present() {
								// SAFETY: We can assume the address is valid if it's been placed in here.
								let l2 = unsafe {
									Phys::from_address_unchecked(l1_entry.address())
										.as_mut_unchecked::<PageTable>()
								};
								for l2_idx in 0..=511 {
									let l2_entry = l2[l2_idx];
									if l2_entry.present() {
										// SAFETY: We can assume the address is valid if it's been placed in here.
										let l3 = unsafe {
											Phys::from_address_unchecked(l2_entry.address())
												.as_mut_unchecked::<PageTable>()
										};
										for l3_idx in 0..=511 {
											let l3_entry = l3[l3_idx];
											if l3_entry.present() {
												// SAFETY: We can assume the address is valid if it's been placed in here.
												let l4 = unsafe {
													Phys::from_address_unchecked(l3_entry.address())
														.as_mut_unchecked::<PageTable>()
												};
												for l4_idx in 0..=511 {
													let l4_entry = l4[l4_idx];
													if l4_entry.present() {
														// SAFETY: We're sure this is a page we want to free.
														unsafe {
															alloc.free(l4_entry.address());
														}
													}
												}
												// SAFETY: We're sure this is a page we want to free.
												unsafe {
													alloc.free(l3_entry.address());
												}
											}
										}
										// SAFETY: We're sure this is a page we want to free.
										unsafe {
											alloc.free(l2_entry.address());
										}
									}
								}
								// SAFETY: We're sure this is a page we want to free.
								unsafe {
									alloc.free(l1_entry.address());
								}
							}
						}
						// SAFETY: We're sure this is a page we want to free.
						unsafe {
							alloc.free(l0_entry.address());
						}
					}
				}
			}
		}

		// Free the page table itself.
		unsafe {
			alloc.free(space.base_phys);
		}
	}

	fn user_thread_stack() -> Self::UserSegment {
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

	fn user_code() -> Self::UserSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new().with_user().with_present(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn user_data() -> Self::UserSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new()
				.with_user()
				.with_present()
				.with_writable()
				.with_no_exec(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn user_rodata() -> Self::UserSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: AddressSpaceLayout::MODULE_EXE_IDX,
			entry_template: PageTableEntry::new()
				.with_user()
				.with_present()
				.with_no_exec(),
			intermediate_entry_template: MODULE_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn sysabi() -> Self::UserSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (AddressSpaceLayout::SYSABI, AddressSpaceLayout::SYSABI),
			entry_template: PageTableEntry::new()
				.with_user()
				.with_present()
				.with_no_exec(),
			intermediate_entry_template: PageTableEntry::new().with_present().with_no_exec(),
		};

		&DESCRIPTOR
	}

	fn kernel_code() -> Self::SupervisorSegment {
		const DESCRIPTOR: AddressSegment = AddressSegment {
			valid_range: (
				AddressSpaceLayout::KERNEL_EXE_IDX,
				AddressSpaceLayout::KERNEL_EXE_IDX,
			),
			entry_template: PageTableEntry::new().with_global().with_present(),
			intermediate_entry_template: KERNEL_EXE_INTERMEDIATE_ENTRY,
		};

		&DESCRIPTOR
	}

	fn kernel_data() -> Self::SupervisorSegment {
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
