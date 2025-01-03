//! Concrete architecture-specific kernel thread handles.

use core::mem::ManuallyDrop;

use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use crate::mem::address_space::{AddressSpaceHandle, AddressSpaceLayout};

/// Concrete architecture-specific kernel thread handle.
pub struct ThreadHandle {
	/// The thread's mapper.
	pub mapper:        ManuallyDrop<AddressSpaceHandle>,
	/// The thread's interrupt stack pointer.
	pub irq_stack_ptr: usize,
	/// The thread's entry point.
	pub entry_point:   usize,
	/// The thread's stack pointer.
	pub stack_ptr:     usize,
}

impl ThreadHandle {
	/// Prepares interrupt stack mappings for the thread upon creation.
	fn prepare_mappings(&mut self) -> Result<(), MapError> {
		// Map only a page for the interrupt stack, with a stack guard.
		//
		// NOTE(qix-): This is NOT the thread's stack, but a separate stack for
		// NOTE(qix-): handling interrupts.
		let irq_stack_segment = AddressSpaceLayout::interrupt_stack();
		let stack_high_guard = irq_stack_segment.range().1 & !0xFFF;
		let stack_start = stack_high_guard - 0x1000;
		let stack_low_guard = stack_start - 0x1000;

		self.irq_stack_ptr = stack_high_guard;

		// Make sure the guard pages are unmapped.
		// More of a debug check, as this should never be the case
		// with a bug-free implementation.
		{
			use oro_mem::mapper::UnmapError;

			match irq_stack_segment.unmap(&self.mapper, stack_high_guard) {
				Ok(phys) => panic!("interrupt stack high guard was already mapped at {phys:016X}"),
				Err(UnmapError::NotMapped) => {}
				Err(err) => {
					panic!("interrupt stack high guard encountered error when unmapping: {err:?}")
				}
			}

			match irq_stack_segment.unmap(&self.mapper, stack_low_guard) {
				Ok(phys) => panic!("interrupt stack low guard was already mapped at {phys:016X}"),
				Err(UnmapError::NotMapped) => {}
				Err(err) => {
					panic!("interrupt stack low guard encountered error when unmapping: {err:?}")
				}
			}
		}

		// Map the stack page.
		let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
		irq_stack_segment.map(&self.mapper, stack_start, phys)?;

		// Now write the initial `iretq` information to the frame.
		// SAFETY(qix-): We know that these are valid addresses.
		unsafe {
			let page_slice = core::slice::from_raw_parts_mut(
				Phys::from_address_unchecked(phys).as_mut_ptr_unchecked(),
				4096 >> 3,
			);
			let written = crate::task::initialize_user_irq_stack(
				page_slice,
				self.entry_point as u64,
				self.stack_ptr as u64,
			);
			self.irq_stack_ptr -= written as usize;
		}

		Ok(())
	}
}

unsafe impl oro_kernel::arch::ThreadHandle<crate::Arch> for ThreadHandle {
	fn new(
		mapper: AddressSpaceHandle,
		stack_ptr: usize,
		entry_point: usize,
	) -> Result<Self, MapError> {
		let mut r = Self {
			irq_stack_ptr: 0,
			mapper: ManuallyDrop::new(mapper),
			entry_point,
			stack_ptr,
		};

		// NOTE(qix-): If it fails, it'll still be dropped.
		r.prepare_mappings()?;

		Ok(r)
	}

	fn mapper(&self) -> &AddressSpaceHandle {
		&self.mapper
	}

	fn migrate(&self) {
		let mapper = crate::Kernel::get().mapper();

		// Re-map the stack and core-local mappings.
		// SAFETY(qix-): We don't need to reclaim anything so overwriting the mappings
		// SAFETY(qix-): is safe.
		unsafe {
			let thread_mapper = self.mapper();
			AddressSpaceLayout::kernel_stack().mirror_into(thread_mapper, mapper);
			AddressSpaceLayout::kernel_core_local().mirror_into(thread_mapper, mapper);
		}
	}
}

impl Drop for ThreadHandle {
	fn drop(&mut self) {
		// SAFETY: The interrupt stack space is fully reclaimable and never shared.
		// SAFETY:
		// SAFETY: Further, the kernel has already reclaimed anything that isn't specifically ours;
		// SAFETY: we must now free the handle itself (without reclaim). This is specified by the
		// SAFETY: `ThreadHandle` trait in the kernel.
		unsafe {
			AddressSpaceLayout::interrupt_stack().unmap_all_and_reclaim(&self.mapper);
			AddressSpaceLayout::free_user_space_handle(ManuallyDrop::take(&mut self.mapper));
		}
	}
}
