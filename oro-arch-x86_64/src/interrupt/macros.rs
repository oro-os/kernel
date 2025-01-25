//! Macros for generating ISRs.

use core::cell::UnsafeCell;

/// An incredibly stupid, unsafe type that forces `Sync` on a type.
///
/// **This is a very unsafe type. Do not use it liberally.**
#[repr(transparent)]
pub(super) struct UnsafeSync<T>(UnsafeCell<T>);

// SAFETY: There isn't any. This isn't safe. We at least try to enforce it
// SAFETY: through an `unsafe` block.
unsafe impl<T> Sync for UnsafeSync<T> {}

impl<T> UnsafeSync<T> {
	/// Creates a new `UnsafeSync` with the given value.
	pub const fn new(value: T) -> Self {
		Self(UnsafeCell::new(value))
	}

	/// Gets a reference to the inner value.
	///
	/// # Safety
	/// **Do not call this from multiple threads under any circumstances.**
	/// Doing so is immediate UB. This is already toeing the line with UB,
	/// but there's not much else we can do here since this is used for
	/// the IDT.
	pub unsafe fn get(&self) -> &T {
		&*self.0.get()
	}
}

/// Aligns a `T` value to 16 bytes.
#[repr(C, align(16))]
pub(super) struct Aligned16<T: Sized>(pub T);

/// Defines the given ISRs in an IDT.
///
/// The ISR functions must have been created with the [`crate::isr!`] macro.
#[macro_export]
macro_rules! isr_table {
	($(#[$meta:meta])* static $isr_table:ident = { $($isr_const:ident[$isr_num:expr] => $isr_name:ident),* , _ => $def_isr_name:ident $(,)? };) => {
		$(mod $isr_name;)*
		mod $def_isr_name;

		$(
			#[doc = concat!("The ISR number for the ", stringify!($isr_name), " ISR.")]
			pub const $isr_const: u8 = $isr_num;
		)*

		// BEG(qix-): Forgive me for this astrocity.
		$(#[$meta])* static $isr_table:
				$crate::interrupt::macros::UnsafeSync<
				::core::cell::LazyCell<
				$crate::interrupt::macros::Aligned16<
				[$crate::interrupt::IdtEntry; 256]
		>>> = $crate::interrupt::macros::UnsafeSync::new(::core::cell::LazyCell::new(|| {
			let mut arr = [
				$crate::interrupt::IdtEntry::new()
					.with_kernel_cs()
					.with_attributes(0x8E)
					.with_isr($def_isr_name::$def_isr_name);
				256
			];

			$(
				arr[$isr_num as usize] = $crate::interrupt::IdtEntry::new()
					.with_kernel_cs()
					.with_attributes(0x8E)
					.with_isr($isr_name::$isr_name);
			)*

			$crate::interrupt::macros::Aligned16(arr)
		}));
	}
}

/// Defines an ISR (Interrupt Service Routine) that will be called by the kernel
/// when the corresponding interrupt is triggered.
#[macro_export]
macro_rules! isr {
	// NOTE(qix-): "@" prefixed match patterns are PRIVATE. Do not use them publicly.
	(@ $isr_name:ident, $err_code:ident) => {
		::oro_macro::paste! {
			$crate::isr_store_task_and_jmp_err!($isr_name %% _rust);
		}
	};

	(@ $isr_name:ident) => {
		::oro_macro::paste! {
			$crate::isr_store_task_and_jmp!($isr_name %% _rust);
		}
	};

	($(#[$meta:meta])* unsafe fn $isr_name:ident($kernel:ident, $user_task:ident $(, $err_code:ident)?) -> Option<Switch> $blk:block) => {
		#[doc = concat!("The ISR (Interrupt Service Routine) trampoline stub for [`", stringify!($isr_name), "_rust`].")]
		#[naked]
		#[no_mangle]
		pub unsafe extern "C" fn $isr_name() -> ! {
			$crate::isr!(@ $isr_name $(, $err_code)?);
		}

		::oro_macro::paste! {
			$(#[$meta])*
			#[no_mangle]
			#[allow(clippy::used_underscore_binding)]
			unsafe extern "C" fn $isr_name %% _rust() -> ! {
				// Must be first.
				let irq_stack_ptr: u64;
				::core::arch::asm!("", out("rcx") irq_stack_ptr, options(nostack, preserves_flags));

				$(
					let $err_code = {
						let err_code: u64;
						::core::arch::asm!("", out("rdx") err_code, options(nostack, preserves_flags));
						err_code
					};
				)?

				let $kernel = $crate::Kernel::get();

				let $user_task = {
					use ::oro_sync::Lock;
					let scheduler_lock = $kernel.scheduler().lock();

					// If this is `None`, then the kernel is currently running.
					// Otherwise it's a userspace task that we just jumped from.
					if let Some(user_task) = scheduler_lock.current_thread().as_ref() {
						user_task.with_mut(|t| t.handle_mut().irq_stack_ptr = irq_stack_ptr as usize);
						drop(scheduler_lock);
						Some(user_task.clone())
					} else {
						$kernel.handle().kernel_irq_stack.get().write(irq_stack_ptr);
						drop(scheduler_lock);
						None
					}
				};

				let switch: Option<::oro_kernel::scheduler::Switch<$crate::Arch>> = $blk;

				let switch = match (switch, $user_task) {
					(Some(s), _) => s,
					(None, Some(user_task)) => {
						::oro_kernel::scheduler::Switch::UserResume(user_task, None)
					}
					(None, None) => {
						::oro_kernel::scheduler::Switch::KernelResume
					}
				};

				match switch {
					::oro_kernel::scheduler::Switch::KernelResume => {
						let kernel_irq_stack = $kernel.handle().kernel_irq_stack.get().read();
						let kernel_stack = $kernel.handle().kernel_stack.get().read();
						::core::arch::asm! {
							"mov rsp, rcx",
							"jmp oro_x86_64_return_to_kernel",
							in("rcx") kernel_irq_stack,
							in("r9") kernel_stack,
							options(noreturn),
						};
					}
					::oro_kernel::scheduler::Switch::UserToKernel => {
						let kernel_irq_stack = $kernel.handle().kernel_irq_stack.get().read();
						let kernel_stack = $kernel.handle().kernel_stack.get().read();
						let kernel_cr3 = $kernel.mapper().base_phys;

						::core::arch::asm! {
							"mov cr3, rdx",
							"mov rsp, rcx",
							"jmp oro_x86_64_return_to_kernel",
							in("rcx") kernel_irq_stack,
							in("r9") kernel_stack,
							in("rdx") kernel_cr3,
							options(noreturn),
						};
					}
					::oro_kernel::scheduler::Switch::UserResume(user_ctx, None)
					| ::oro_kernel::scheduler::Switch::UserToUser(user_ctx, None)
					| ::oro_kernel::scheduler::Switch::KernelToUser(user_ctx, None) => {
						let (thread_cr3_phys, thread_rsp) = unsafe {
							user_ctx.with(|ctx_lock| {
								use ::oro_mem::mapper::AddressSegment;

								let mapper = ctx_lock.mapper();
								let cr3 = mapper.base_phys;
								let rsp = ctx_lock.handle().irq_stack_ptr;
								(*$kernel.handle().tss.get())
									.rsp0
									.write($crate::mem::address_space::AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
								(cr3, rsp)
							})
						};

						drop(user_ctx);

						::core::arch::asm! {
							"jmp oro_x86_64_user_to_user",
							in("rax") thread_cr3_phys,
							in("rdx") thread_rsp,
							options(noreturn),
						};
					}

					::oro_kernel::scheduler::Switch::UserResume(user_ctx, Some(syscall_response))
					| ::oro_kernel::scheduler::Switch::UserToUser(user_ctx, Some(syscall_response))
					| ::oro_kernel::scheduler::Switch::KernelToUser(user_ctx, Some(syscall_response)) => {
						let (thread_cr3_phys, thread_rsp) = unsafe {
							user_ctx.with(|ctx_lock| {
								use ::oro_mem::mapper::AddressSegment;

								let mapper = ctx_lock.mapper();
								let cr3 = mapper.base_phys;
								let rsp = ctx_lock.handle().irq_stack_ptr;
								(*$kernel.handle().tss.get())
									.rsp0
									.write($crate::mem::address_space::AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
								(cr3, rsp)
							})
						};

						drop(user_ctx);

						::core::arch::asm! {
							"jmp oro_x86_64_user_to_user_sysret",
							in("r8") thread_cr3_phys,
							in("r10") thread_rsp,
							in("rax") syscall_response.error as u64,
							in("r9") syscall_response.ret,
							options(noreturn)
						}
					}
				}
			}
		}
	};
}
