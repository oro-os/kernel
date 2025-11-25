//! Concrete architecture-specific kernel thread handles.

use core::{cell::UnsafeCell, mem::ManuallyDrop, ptr::null_mut};

use oro_kernel::event::SystemCallResponse;
use oro_kernel_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use crate::{
	interrupt::StackFrame,
	mem::address_space::{AddressSpaceHandle, AddressSpaceLayout},
};

/// Unsafe `Sync`/`Send` wrapper. There's no way to implement
/// the IRQ stack pointer otherwise without using a `Mutex`, but
/// a mutex's lock cannot be held across context switch boundaries.
/// Further, this is initialized once.
#[repr(transparent)]
struct Unsafe<T>(*mut T);

unsafe impl<T> Sync for Unsafe<T> {}
unsafe impl<T> Send for Unsafe<T> {}

/// Concrete architecture-specific kernel thread handle.
pub struct ThreadHandle {
	/// The thread's mapper.
	mapper:      ManuallyDrop<AddressSpaceHandle>,
	/// The pointer to the base of the [`StackFrame`]
	/// that is saved/restored when context switching to this
	/// thread, **in kernel-space**.
	///
	/// **DO NOT USE THIS AS THE STACK POINTER WHEN SWITCHING
	/// TO THE USERSPACE CONTEXT.**
	stack_frame: Unsafe<UnsafeCell<StackFrame>>,
}

impl ThreadHandle {
	/// Performs an `iret` back into this thread's userspace context.
	///
	/// # Safety
	/// See [`crate::interrupt::isr::iret_context`] for safety considerations.
	#[inline]
	pub unsafe fn iret(&self) -> ! {
		crate::interrupt::isr::iret_context(self.mapper.base_phys)
	}

	/// Performs a `sysret` back into this thread's userspace context.
	///
	/// # Safety
	/// See [`crate::syscall::sysret_context`] for safety considerations.
	#[inline]
	pub unsafe fn sysret(&self, res: &SystemCallResponse) -> ! {
		crate::syscall::sysret_context(self.mapper.base_phys, res);
	}

	/// Returns the current `fsbase` denoted in the thread's stack frame.
	///
	/// Note that if the thread is running and this method is being called
	/// from a different core, it is _technically_ safe but there is no guarantee
	/// about the value returned from this function.
	///
	/// **Callers must not use this value for any pointer or reference purposes
	/// as it is _entirely_ user controlled, racey, and unreliable. Do not try
	/// to interpret this value.**
	#[must_use]
	#[inline]
	pub fn fsbase(&self) -> u64 {
		// SAFETY: We can more or less ignore any problems with this as the caller
		// SAFETY: should not be using this in any way that is consequential.
		unsafe { ::core::ptr::read_volatile(&raw const (*(*self.stack_frame.0).get()).fsbase) }
	}

	/// Returns the current `gsbase` denoted in the thread's stack frame.
	///
	/// Note that if the thread is running and this method is being called
	/// from a different core, it is _technically_ safe but there is no guarantee
	/// about the value returned from this function.
	///
	/// **Callers must not use this value for any pointer or reference purposes
	/// as it is _entirely_ user controlled, racey, and unreliable. Do not try
	/// to interpret this value.**
	#[must_use]
	#[inline]
	pub fn gsbase(&self) -> u64 {
		// SAFETY: We can more or less ignore any problems with this as the caller
		// SAFETY: should not be using this in any way that is consequential.
		unsafe { ::core::ptr::read_volatile(&raw const (*(*self.stack_frame.0).get()).gsbase) }
	}

	/// Sets the thread's FS base.
	///
	/// Will be picked up on the next context switch.
	///
	/// # Safety
	/// Calling this function from another core while the thread
	/// is executing may cause problems for the userspace program. It's not
	/// recommended to do this unless _this thread_ has asked for it.
	pub unsafe fn set_fsbase(&self, value: u64) {
		// SAFETY: Safety considerations offloaded to the caller.
		unsafe {
			::core::ptr::write_volatile(&raw mut (*(*self.stack_frame.0).get()).fsbase, value);
		}
	}

	/// Sets the thread's GS base.
	///
	/// Will be picked up on the next context switch.
	///
	/// # Safety
	/// Calling this function from another core while the thread
	/// is executing may cause problems for the userspace program. It's not
	/// recommended to do this unless _this thread_ has asked for it.
	pub unsafe fn set_gsbase(&self, value: u64) {
		// SAFETY: Safety considerations offloaded to the caller.
		unsafe {
			::core::ptr::write_volatile(&raw mut (*(*self.stack_frame.0).get()).gsbase, value);
		}
	}
}

unsafe impl oro_kernel::arch::ThreadHandle<crate::Arch> for ThreadHandle {
	fn new(
		mapper: AddressSpaceHandle,
		stack_ptr: usize,
		entry_point: usize,
	) -> Result<Self, MapError> {
		let mut r = Self {
			mapper:      ManuallyDrop::new(mapper),
			stack_frame: Unsafe(null_mut()),
		};

		// Map in pages for the interrupt stack, with a stack guard.
		//
		// NOTE(qix-): This is NOT the thread's stack, but a separate stack for
		// NOTE(qix-): handling interrupts.
		let irq_stack_segment = AddressSpaceLayout::interrupt_stack();
		let irq_stack_high_guard = irq_stack_segment.range().1 & !0xFFF;

		match irq_stack_segment.unmap(&r.mapper, irq_stack_high_guard) {
			Ok(phys) => panic!("interrupt stack high guard was already mapped at {phys:016X}"),
			Err(UnmapError::NotMapped) => {}
			Err(err) => {
				panic!("interrupt stack high guard encountered error when unmapping: {err:?}")
			}
		}

		let irq_stack_user_virt = irq_stack_high_guard - 0x1000;

		let irq_stack_phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
		irq_stack_segment.map(&r.mapper, irq_stack_user_virt, irq_stack_phys)?;

		// Make sure the guard pages are unmapped.
		// More of a debug check, as this should never be the case
		// with a bug-free implementation.
		let irq_stack_lower_guard = irq_stack_user_virt - 0x1000;
		match irq_stack_segment.unmap(&r.mapper, irq_stack_lower_guard) {
			Ok(phys) => panic!("interrupt stack low guard was already mapped at {phys:016X}"),
			Err(UnmapError::NotMapped) => {}
			Err(err) => {
				panic!("interrupt stack low guard encountered error when unmapping: {err:?}")
			}
		}

		// Now write the initial `iretq` information to the frame.
		// SAFETY(qix-): We know that these are valid addresses.
		unsafe {
			r.stack_frame.0 = Phys::from_address_unchecked(
				irq_stack_phys + (0x1000 - size_of::<StackFrame>()) as u64,
			)
			.as_mut_ptr_unchecked();

			debug_assert!(r.stack_frame.0.is_aligned());

			r.stack_frame.0.write_volatile(UnsafeCell::new(StackFrame {
				cs: u64::from(oro_arch_x86_64::gdt::USER_CS | 3),
				ss: u64::from(oro_arch_x86_64::gdt::USER_DS | 3),
				sp: stack_ptr as u64,
				ip: entry_point as u64,
				// TODO(qix-): Set up a bitstruct for this
				flags: 0x2 | 0x200 | 0x0004_0000 | 0x0001_0000,
				..Default::default()
			}));
		}

		debug_assert_ne!(r.stack_frame.0, null_mut());
		// SAFETY: We should have initialized this already, I'm just sanity-checking assumptions here.
		debug_assert_eq!(r.stack_frame.0.cast(), unsafe { (*r.stack_frame.0).get() });
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
