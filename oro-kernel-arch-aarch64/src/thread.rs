//! Concrete architecture-specific kernel thread handles.

use core::mem::ManuallyDrop;

use oro_kernel_mem::mapper::{AddressSpace, MapError};

use crate::mem::address_space::{AddressSpaceLayout, Ttbr0Handle};

/// Concrete architecture-specific kernel thread handle.
pub struct ThreadHandle {
	/// The thread's mapper.
	pub mapper:      ManuallyDrop<Ttbr0Handle>,
	/// The thread's entry point.
	pub entry_point: usize,
	/// The thread's stack pointer.
	pub stack_ptr:   usize,
}

unsafe impl oro_kernel::arch::ThreadHandle<crate::Arch> for ThreadHandle {
	fn new(mapper: Ttbr0Handle, stack_ptr: usize, entry_point: usize) -> Result<Self, MapError> {
		Ok(Self {
			mapper: ManuallyDrop::new(mapper),
			entry_point,
			stack_ptr,
		})
	}

	fn mapper(&self) -> &Ttbr0Handle {
		&self.mapper
	}

	fn migrate(&self) {
		// No-op
	}
}

impl Drop for ThreadHandle {
	fn drop(&mut self) {
		// SAFETY: The kernel has already reclaimed anything that isn't specifically ours;
		// SAFETY: we must now free the handle itself (without reclaim). This is specified by the
		// SAFETY: `ThreadHandle` trait in the kernel.
		unsafe {
			AddressSpaceLayout::free_user_space_handle(ManuallyDrop::take(&mut self.mapper));
		}
	}
}
