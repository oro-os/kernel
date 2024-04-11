//! An implementation of a recursive page table mapper for x86_64.
#![allow(clippy::inline_always)]

use crate::mem::{
	layout::{Descriptor, Layout},
	paging::{PageTable, PageTableEntry},
	paging_level::PagingLevel,
};
use core::ptr::from_ref;
use oro_common::{
	critical_section,
	mem::{
		AddressSpace, MapError, PageFrameAllocate, PageFrameFree, RuntimeAddressSpace,
		SupervisorAddressSegment, SupervisorAddressSpace, UnmapError,
	},
	unlikely, unsafe_precondition,
};

/// Encodes a set of page table indices into a mutable references
/// to a [`PageTableEntry`] or [`PageTable`] for 4 level paging.
///
/// # Safety
/// Indices must be in the range 0..512 and be usize typed.
macro_rules! encode_to_entry_l4 {
	($L4:expr, $L3:expr, $L2:expr, $L1:expr) => {{
		let vaddr: usize = (Layout::RECURSIVE_IDX << 39) | ($L4 << 30) | ($L3 << 21) | ($L2 << 12);
		let vaddr: usize = vaddr | ($L1 << 3);
		let vaddr: usize = (((vaddr << 16) as isize) >> 16) as usize;
		&mut *(vaddr as *mut _)
	}};
	($L4:expr, $L3:expr, $L2:expr) => {
		encode_to_entry_l4!(Layout::RECURSIVE_IDX, $L4, $L3, $L2)
	};
	($L4:expr, $L3:expr) => {
		encode_to_entry_l4!(Layout::RECURSIVE_IDX, $L4, $L3)
	};
	($L4:expr) => {
		encode_to_entry_l4!(Layout::RECURSIVE_IDX, $L4)
	};
}

/// Encodes a set of page table indices into a mutable references
/// to a [`PageTableEntry`] or [`PageTable`] for 5 level paging.
///
/// # Safety
/// Indices must be in the range 0..512 and be usize typed.
macro_rules! encode_to_entry_l5 {
	($L5:expr, $L4:expr, $L3:expr, $L2:expr, $L1:expr) => {{
		let vaddr: usize =
			(Layout::RECURSIVE_IDX << 48) | ($L5 << 39) | ($L4 << 30) | ($L3 << 21) | ($L2 << 12);
		let vaddr: usize = vaddr | ($L1 << 3);
		let vaddr: usize = (((vaddr << 7) as isize) >> 7) as usize;
		&mut *(vaddr as *mut _)
	}};
	($L5:expr, $L4:expr, $L3:expr, $L2:expr) => {
		encode_to_entry_l5!(Layout::RECURSIVE_IDX, $L5, $L4, $L3, $L2)
	};
	($L5:expr, $L4:expr, $L3:expr) => {
		encode_to_entry_l5!(Layout::RECURSIVE_IDX, $L5, $L4, $L3)
	};
	($L5:expr, $L4:expr) => {
		encode_to_entry_l5!(Layout::RECURSIVE_IDX, $L5, $L4)
	};
	($L5:expr) => {
		encode_to_entry_l5!(Layout::RECURSIVE_IDX, $L5)
	};
}

/// Provides the kernel's runtime mapper in the form of a recursive page table.
pub struct RecursiveMapper {
	/// The current CR3 value as tracked by this mapper.
	current_cr3:  u64,
	/// The paging level of the CPU.
	paging_level: PagingLevel,
}

impl RecursiveMapper {
	/// Ensures that the `RECURSIVE_IDX` is less than 512 so as not to go
	/// out of range of the page table.
	#[allow(clippy::assertions_on_constants)]
	const ASSERT_INDEX: () = assert!(
		Layout::RECURSIVE_IDX < 512,
		"Layout::RECURSIVE_IDX must be less than 512"
	);

	/// Performs the index assertion (should be called somewhere in the address space implementation).
	#[inline(always)]
	fn assert_index() {
		let _: () = Self::ASSERT_INDEX;
	}
}

unsafe impl AddressSpace for RecursiveMapper {
	type Layout = Layout;
}

impl SupervisorAddressSpace for RecursiveMapper {
	type Segment<'a> = RecursiveSupervisorSegment where Self: 'a;

	fn for_supervisor_segment(&self, descriptor: &'static Descriptor) -> Self::Segment<'_> {
		RecursiveSupervisorSegment {
			paging_level: self.paging_level,
			descriptor,
		}
	}
}

unsafe impl RuntimeAddressSpace for RecursiveMapper {
	// We just use physical addresses for the recursive mapper.
	type AddressSpaceHandle = u64;

	#[cold]
	unsafe fn take() -> Self {
		// No-op; performs the const assertion that the recursive index is less than 512.
		Self::assert_index();

		// Get the paging level for the CPU
		let paging_level = PagingLevel::current_from_cpu();

		// Get the recursive page table entry
		let recursive_entry: &mut PageTableEntry = match paging_level {
			PagingLevel::Level4 => encode_to_entry_l4!(Layout::RECURSIVE_IDX),
			PagingLevel::Level5 => encode_to_entry_l5!(Layout::RECURSIVE_IDX),
		};

		// Make sure that the recursive page table entry
		// is present.
		let cr3 = crate::asm::cr3();
		unsafe_precondition!(
			crate::X86_64,
			recursive_entry.present(),
			"recursive page table entry must be present"
		);
		unsafe_precondition!(
			crate::X86_64,
			recursive_entry.address() == cr3,
			"recursive page table entry must point to the current page table"
		);

		// Make sure the recursive index is marked as no-user and no-execute.
		recursive_entry.clear_user();
		recursive_entry.set_no_exec();

		Self {
			current_cr3: cr3,
			paging_level,
		}
	}

	#[inline(always)]
	fn handle(&self) -> Self::AddressSpaceHandle {
		unsafe {
			critical_section!(crate::X86_64, {
				unsafe_precondition!(
					crate::X86_64,
					crate::asm::cr3() == self.current_cr3,
					"cr3 mismatch!"
				);
				self.current_cr3
			})
		}
	}

	#[inline(always)]
	unsafe fn make_active(&mut self, mut handle: Self::AddressSpaceHandle) -> u64 {
		critical_section!(crate::X86_64, {
			unsafe_precondition!(
				crate::X86_64,
				crate::asm::cr3() == self.current_cr3,
				"cr3 mismatch!"
			);

			if self.current_cr3 != handle {
				crate::asm::set_cr3(handle);
				core::mem::swap(&mut self.current_cr3, &mut handle);
			}

			handle
		})
	}
}

/// The supervisor segment type for the recursive mapper.
pub struct RecursiveSupervisorSegment {
	/// The paging level of the CPU.
	paging_level: PagingLevel,
	/// The segment descriptor for this segment.
	descriptor:   &'static Descriptor,
}

unsafe impl SupervisorAddressSegment for RecursiveSupervisorSegment {
	fn map<A>(&mut self, allocator: &mut A, virt: usize, phys: u64) -> Result<(), MapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(MapError::VirtNotAligned);
		}

		{
			let root_index = (virt >> 39) & 0x1FF;
			if unlikely!(
				root_index < self.descriptor.valid_range.0
					|| root_index > self.descriptor.valid_range.1
			) {
				return Err(MapError::VirtOutOfRange);
			}
		}

		// TODO(qix-): There's probably a good way of doing this without
		// TODO(qix-): doing all of the shifts. I'm going to put these
		// TODO(qix-): here in an incredibly unsafe way for now in hopes
		// TODO(qix-): that the compiler will optimize these into something
		// TODO(qix-): a little less verbose.
		// TODO(qix-):
		// TODO(qix-): For now, please make sure when modifying this code
		// TODO(qix-): that none of these are dereferenced until the previous
		// TODO(qix-): level has been checked.
		match self.paging_level {
			PagingLevel::Level4 => {
				let l4_idx = (virt >> 39) & 0x1FF;
				let l3_idx = (virt >> 30) & 0x1FF;
				let l2_idx = (virt >> 21) & 0x1FF;
				let l1_idx = (virt >> 12) & 0x1FF;

				let l4_entry: &mut PageTableEntry = unsafe { encode_to_entry_l4!(l4_idx) };
				if !l4_entry.present() {
					let l3_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l4_entry = self.descriptor.entry_template.with_address(l3_phys);
					let l3_pt: &mut PageTable = unsafe { encode_to_entry_l4!(l4_idx, 0) };
					l3_pt.reset();
				}

				let l3_entry: &mut PageTableEntry = unsafe { encode_to_entry_l4!(l4_idx, l3_idx) };
				if !l3_entry.present() {
					let l2_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l3_entry = self.descriptor.entry_template.with_address(l2_phys);
					let l2_pt: &mut PageTable = unsafe { encode_to_entry_l4!(l4_idx, l3_idx, 0) };
					l2_pt.reset();
				}

				let l2_entry: &mut PageTableEntry =
					unsafe { encode_to_entry_l4!(l4_idx, l3_idx, l2_idx) };
				if !l2_entry.present() {
					let l1_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l2_entry = self.descriptor.entry_template.with_address(l1_phys);
					let l1_pt: &mut PageTable =
						unsafe { encode_to_entry_l4!(l4_idx, l3_idx, l2_idx, 0) };
					l1_pt.reset();
				}

				let l1_entry: &mut PageTableEntry =
					unsafe { encode_to_entry_l4!(l4_idx, l3_idx, l2_idx, l1_idx) };
				if l1_entry.present() {
					return Err(MapError::Exists);
				}

				*l1_entry = self.descriptor.entry_template.with_address(phys);
			}
			PagingLevel::Level5 => {
				let l5_idx = (virt >> 48) & 0x1FF;
				let l4_idx = (virt >> 39) & 0x1FF;
				let l3_idx = (virt >> 30) & 0x1FF;
				let l2_idx = (virt >> 21) & 0x1FF;
				let l1_idx = (virt >> 12) & 0x1FF;

				let l5_entry: &mut PageTableEntry = unsafe { encode_to_entry_l5!(l5_idx) };
				if !l5_entry.present() {
					let l4_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l5_entry = self.descriptor.entry_template.with_address(l4_phys);
					let l4_pt: &mut PageTable = unsafe { encode_to_entry_l5!(l5_idx, 0) };
					l4_pt.reset();
				}

				let l4_entry: &mut PageTableEntry = unsafe { encode_to_entry_l5!(l5_idx, l4_idx) };
				if !l4_entry.present() {
					let l3_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l4_entry = self.descriptor.entry_template.with_address(l3_phys);
					let l3_pt: &mut PageTable = unsafe { encode_to_entry_l5!(l5_idx, l4_idx, 0) };
					l3_pt.reset();
				}

				let l3_entry: &mut PageTableEntry =
					unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx) };
				if !l3_entry.present() {
					let l2_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l3_entry = self.descriptor.entry_template.with_address(l2_phys);
					let l2_pt: &mut PageTable =
						unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, 0) };
					l2_pt.reset();
				}

				let l2_entry: &mut PageTableEntry =
					unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, l2_idx) };
				if !l2_entry.present() {
					let l1_phys = allocator.allocate().ok_or(MapError::OutOfMemory)?;
					*l2_entry = self.descriptor.entry_template.with_address(l1_phys);
					let l1_pt: &mut PageTable =
						unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, l2_idx, 0) };
					l1_pt.reset();
				}

				let l1_entry: &mut PageTableEntry =
					unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, l2_idx, l1_idx) };
				if l1_entry.present() {
					return Err(MapError::Exists);
				}

				*l1_entry = self.descriptor.entry_template.with_address(phys);
			}
		}

		Ok(())
	}

	#[allow(clippy::too_many_lines)]
	fn unmap<A>(&mut self, allocator: &mut A, virt: usize) -> Result<u64, UnmapError>
	where
		A: PageFrameAllocate + PageFrameFree,
	{
		if unlikely!(virt & 0xFFF != 0) {
			return Err(UnmapError::VirtNotAligned);
		}

		{
			let root_index = (virt >> 39) & 0x1FF;
			if unlikely!(
				root_index < self.descriptor.valid_range.0
					|| root_index > self.descriptor.valid_range.1
			) {
				return Err(UnmapError::VirtOutOfRange);
			}
		}

		// TODO(qix-): See the comment in the `map` function regarding
		// TODO(qix-): the verbosity of these shifts.
		match self.paging_level {
			PagingLevel::Level4 => {
				let l4_idx = (virt >> 39) & 0x1FF;
				let l3_idx = (virt >> 30) & 0x1FF;
				let l2_idx = (virt >> 21) & 0x1FF;
				let l1_idx = (virt >> 12) & 0x1FF;

				let l4: &mut PageTable = unsafe { encode_to_entry_l4!(0) };
				let l4_entry = &mut l4[l4_idx];
				if !l4_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l3: &mut PageTable = unsafe { encode_to_entry_l4!(l4_idx, 0) };
				let l3_entry = &mut l3[l3_idx];
				if !l3_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l2: &mut PageTable = unsafe { encode_to_entry_l4!(l4_idx, l3_idx, 0) };
				let l2_entry = &mut l2[l2_idx];
				if !l2_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l1: &mut PageTable = unsafe { encode_to_entry_l4!(l4_idx, l3_idx, l2_idx, 0) };
				let l1_entry = &mut l1[l1_idx];
				if !l1_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let phys = l1_entry.address();
				l1_entry.reset();
				crate::asm::invlpg(virt);

				if l1.empty() {
					unsafe {
						allocator.free(l2_entry.address());
					}
					l2_entry.reset();
					crate::asm::invlpg(from_ref(&l1) as usize);
				}

				if l2.empty() {
					unsafe {
						allocator.free(l3_entry.address());
					}
					l3_entry.reset();
					crate::asm::invlpg(from_ref(&l2) as usize);
				}

				if l3.empty() {
					unsafe {
						allocator.free(l4_entry.address());
					}
					l4_entry.reset();
					crate::asm::invlpg(from_ref(&l3) as usize);
				}

				Ok(phys)
			}
			PagingLevel::Level5 => {
				let l5_idx = (virt >> 48) & 0x1FF;
				let l4_idx = (virt >> 39) & 0x1FF;
				let l3_idx = (virt >> 30) & 0x1FF;
				let l2_idx = (virt >> 21) & 0x1FF;
				let l1_idx = (virt >> 12) & 0x1FF;

				let l5: &mut PageTable = unsafe { encode_to_entry_l5!(0) };
				let l5_entry = &mut l5[l5_idx];
				if !l5_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l4: &mut PageTable = unsafe { encode_to_entry_l5!(l5_idx, 0) };
				let l4_entry = &mut l4[l4_idx];
				if !l4_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l3: &mut PageTable = unsafe { encode_to_entry_l5!(l5_idx, l4_idx, 0) };
				let l3_entry = &mut l3[l3_idx];
				if !l3_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l2: &mut PageTable = unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, 0) };
				let l2_entry = &mut l2[l2_idx];
				if !l2_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let l1: &mut PageTable =
					unsafe { encode_to_entry_l5!(l5_idx, l4_idx, l3_idx, l2_idx, 0) };
				let l1_entry = &mut l1[l1_idx];
				if !l1_entry.present() {
					return Err(UnmapError::NotMapped);
				}

				let phys = l1_entry.address();
				l1_entry.reset();
				crate::asm::invlpg(virt);

				if l1.empty() {
					unsafe {
						allocator.free(l2_entry.address());
					}
					l2_entry.reset();
					crate::asm::invlpg(from_ref(&l1) as usize);
				}

				if l2.empty() {
					unsafe {
						allocator.free(l3_entry.address());
					}
					l3_entry.reset();
					crate::asm::invlpg(from_ref(&l2) as usize);
				}

				if l3.empty() {
					unsafe {
						allocator.free(l4_entry.address());
					}
					l4_entry.reset();
					crate::asm::invlpg(from_ref(&l3) as usize);
				}

				if l4.empty() {
					unsafe {
						allocator.free(l5_entry.address());
					}
					l5_entry.reset();
					crate::asm::invlpg(from_ref(&l4) as usize);
				}

				Ok(phys)
			}
		}
	}
}
