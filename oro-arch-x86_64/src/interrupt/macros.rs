//! Macros for generating ISRs.

use core::{cell::UnsafeCell, mem::MaybeUninit};

use oro_sync::{Lock, Mutex};

use super::IdtEntry;

/// Aligns a `T` value to 16 bytes.
#[repr(C, align(16))]
pub(super) struct Aligned16<T: Sized>(pub T);

/// ISR Table; wrapper around various structures to make it
/// safely initializable and aligned.
#[expect(clippy::type_complexity)]
pub(super) struct IsrTable<Init: FnOnce() -> Aligned16<[IdtEntry; 256]>>(
	Mutex<(
		UnsafeCell<MaybeUninit<Aligned16<[IdtEntry; 256]>>>,
		Option<Init>,
	)>,
);

impl<Init: FnOnce() -> Aligned16<[IdtEntry; 256]>> IsrTable<Init> {
	/// Creates a new `IsrTable` with the given initializer when the ISR table is fetched.
	///
	/// **Do not use this function. It's meant only to be called by the `isr_table!` macro.**
	pub const fn new(initializer: Init) -> Self {
		Self(Mutex::new((
			UnsafeCell::new(MaybeUninit::uninit()),
			Some(initializer),
		)))
	}

	/// Returns a pointer to the ISR table.
	///
	/// Do not call in hot paths.
	#[cold]
	pub fn get(&'static self) -> &'static [IdtEntry; 256] {
		let mut lock = self.0.lock();
		if let Some(init) = lock.1.take() {
			// SAFETY: We have exclusive access to the `MaybeUninit`.
			unsafe {
				lock.0.get().write(MaybeUninit::new(init()));
			}
		}

		// SAFETY: We can guarantee it's initialized here and is only being read.
		let ptr = unsafe {
			lock.0
				.get()
				.as_mut()
				.expect("failed to fetch ISR pointer")
				.as_ptr()
				.cast()
		};

		drop(lock);

		// SAFETY: It's only ever being read beyond this point, therefore
		// SAFETY: no locking is necessary.
		unsafe { &*ptr }
	}
}

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
				$crate::interrupt::macros::IsrTable<fn() -> $crate::interrupt::macros::Aligned16<[$crate::interrupt::IdtEntry; 256]>> =
			$crate::interrupt::macros::IsrTable::new(|| {
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
		});
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
		#[unsafe(no_mangle)]
		pub unsafe extern "C" fn $isr_name() -> ! {
			$crate::isr!(@ $isr_name $(, $err_code)?);
		}

		::oro_macro::paste! {
			$(#[$meta])*
			#[unsafe(no_mangle)]
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
						user_task.with_mut(|t| {
							let handle = t.handle_mut();
							handle.irq_stack_ptr = irq_stack_ptr as usize;
							handle.fsbase = $crate::asm::get_fs_msr();
							handle.gsbase = $crate::asm::get_gs_msr();
						});
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

								$crate::asm::set_fs_msr(ctx_lock.handle().fsbase);
								$crate::asm::set_gs_msr(ctx_lock.handle().gsbase);

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

								$crate::asm::set_fs_msr(ctx_lock.handle().fsbase);
								$crate::asm::set_gs_msr(ctx_lock.handle().gsbase);

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

/// Sets up the default ISR table.
#[macro_export]
macro_rules! default_isr_table {
	(
		$(#[$meta:meta])*
		$vis:vis fn $name:ident() -> &'static [IdtEntry; $N:expr] = [
			$($isrty:tt),* $(,)?
		];
	) => {
		::core::arch::global_asm! {
			$(
				concat!(".global _oro_default_isr_", ${index(0)}, "\n"),
				concat!("_oro_default_isr_", ${index(0)}, ":"),
				$crate::default_isr_table!(@ push $isrty),
				concat!("push ", ${index(0)}),
				 // NOTE(qix-): This is not technically stable. RDI is used as the
				 // NOTE(qix-): first argument to a rust function, but that's not
				 // NOTE(qix-): guaranteed to be the case in the future.
				"push rdi",
				"mov rdi, cr0",
				"push rdi",
				"mov rdi, cr2",
				"push rdi",
				"mov rdi, cr3",
				"push rdi",
				"mov rdi, cr4",
				"push rdi",
				"push rax",
				"push rbx",
				"push rcx",
				"push rdx",
				"push rsi",
				"push rbp",
				"push r8",
				"push r9",
				"push r10",
				"push r11",
				"push r12",
				"push r13",
				"push r14",
				"push r15",
				"mov rax, 1",
				"xor rcx, rcx",
				"xor rdx, rdx",
				"xor rbx, rbx",
				"cpuid",
				"shr rbx, 24",
				"and rbx, 0xFF",
				"push rbx",
				"mov rdi, rsp",
				"jmp _oro_default_isr_handler",
				"ud2",
			)*
		}

		$(#[$meta])*
		$vis fn $name() -> &'static [$crate::interrupt::IdtEntry; $N] {
			use oro_sync::Lock;
			static TABLE: oro_sync::Mutex<
				Option<core::cell::UnsafeCell<[$crate::interrupt::IdtEntry; $N]>>
			> = oro_sync::Mutex::new(None);

			if let Some(table) = TABLE.lock().as_ref().map(|t| t.get()) {
				// SAFETY: We can guarantee it's initialized here and is only being read.
				unsafe { &*table }
			} else {
				let new_table = [$(
					$crate::interrupt::IdtEntry::new()
						.with_kernel_cs()
						.with_attributes(0x8E)
						.with_isr({
							unsafe extern "C" {
								#[link_name = concat!("_oro_default_isr_", ::core::stringify!(${ignore($isrty)}${index(0)}))]
								unsafe fn _isr_handler() -> !;
							}

							_isr_handler
						})
				),*];

				let mut table = TABLE.lock();
				// SAFETY: It's only ever being read beyond this point, therefore
				// SAFETY: it's safe to refer to the inner contents statically.
				unsafe { &*table.get_or_insert_with(|| core::cell::UnsafeCell::new(new_table)).get() }
			}
		}
	};

	(@ push error) => { "" };
	(@ push noerror) => { "push 0" };
	(@ push $tt:tt) => { compile_error!("invalid handler type") };
}
