//! Architecture / core initialization
//! routines and global state definitions.

use core::{arch::asm, cell::UnsafeCell, mem::MaybeUninit};

use oro_debug::{dbg, dbg_err, dbg_warn};
use oro_elf::{ElfSegment, ElfSegmentType};
use oro_kernel::{
	KernelState, instance::Instance, module::Module, scheduler::Switch, thread::Thread,
};
use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};
use oro_sync::Lock;

use crate::{
	gdt::{Gdt, SysEntry},
	lapic::Lapic,
	mem::address_space::AddressSpaceLayout,
	tss::Tss,
};

/// The global kernel state. Initialized once during boot
/// and re-used across all cores.
pub static mut KERNEL_STATE: MaybeUninit<KernelState<crate::Arch>> = MaybeUninit::uninit();

/// Initializes the global state of the architecture.
///
/// # Panics
/// Panics if loading root ring modules fails in any way.
///
/// # Safety
/// Must be called exactly once for the lifetime of the system,
/// only by the boot processor at boot time (_not_ at any
/// subsequent bringup).
///
/// Must be called before [`boot()`].
pub unsafe fn initialize_primary(lapic: Lapic) {
	#[cfg(debug_assertions)]
	{
		use core::sync::atomic::{AtomicBool, Ordering};

		#[doc(hidden)]
		static HAS_INITIALIZED: AtomicBool = AtomicBool::new(false);

		if HAS_INITIALIZED
			.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
			.is_err()
		{
			panic!("init() called more than once");
		}
	}

	// SAFETY(qix-): This is the only place we take a mutable reference to it.
	#[expect(static_mut_refs)]
	KernelState::init(&mut KERNEL_STATE).expect("failed to create global kernel state");

	initialize_core_local(lapic);
	let kernel = crate::Kernel::get();

	// TODO(qix-): Not sure that I like that this is ELF-aware. This may get
	// TODO(qix-): refactored at some point.
	if let Some(oro_boot_protocol::modules::ModulesKind::V0(modules)) =
		crate::boot::protocol::MODULES_REQUEST.response()
	{
		let modules = core::ptr::read_volatile(modules.assume_init_ref());
		let mut next = modules.next;

		let root_ring = kernel.state().root_ring();

		while next != 0 {
			let Some(module) =
				Phys::from_address_unchecked(next).as_ref::<oro_boot_protocol::Module>()
			else {
				dbg_err!(
					"failed to load module; invalid address (either null after translation, or \
					 unaligned): {next:016X}"
				);
				continue;
			};

			next = core::ptr::read_volatile(&module.next);

			dbg!("loading module: {:016X} ({})", module.base, module.length);

			let module_handle = Module::new().expect("failed to create root ring module");

			let entry_point = module_handle.with(|module_lock| {
				let mapper = module_lock.mapper();

				let elf_base = Phys::from_address_unchecked(module.base).as_ptr_unchecked::<u8>();
				let elf = oro_elf::Elf::parse(
					elf_base,
					usize::try_from(module.length).unwrap(),
					crate::ELF_ENDIANNESS,
					crate::ELF_CLASS,
					crate::ELF_MACHINE,
				)
				.expect("failed to parse ELF");

				for segment in elf.segments() {
					let mapper_segment = match segment.ty() {
						ElfSegmentType::Ignored => return None,
						ElfSegmentType::Invalid { flags, ptype } => {
							dbg_err!(
								"root ring module has invalid segment; skipping: ptype={ptype:?} \
								 flags={flags:?}",
							);
							return None;
						}
						ElfSegmentType::ModuleCode => AddressSpaceLayout::user_code(),
						ElfSegmentType::ModuleData => AddressSpaceLayout::user_data(),
						ElfSegmentType::ModuleRoData => AddressSpaceLayout::user_rodata(),
						ty => {
							dbg_err!("root ring module has invalid segment {ty:?}; skipping",);
							return None;
						}
					};

					dbg!(
						"loading {:?} segment: {:016X} {:016X} -> {:016X} ({})",
						segment.ty(),
						segment.load_address(),
						segment.load_size(),
						segment.target_address(),
						segment.target_size()
					);

					// NOTE(qix-): This will almost definitely be improved in the future.
					// NOTE(qix-): At the very least, hugepages will change this.
					// NOTE(qix-): There will probably be some better machinery for
					// NOTE(qix-): mapping ranges of memory in the future.
					for page in 0..(segment.target_size().saturating_add(0xFFF) >> 12) {
						let phys_addr = GlobalPfa
							.allocate()
							.expect("failed to map root ring module; out of memory");

						let byte_offset = page << 12;
						// Saturating sub here since the target size might exceed the file size,
						// in which case we have to keep allocating those pages and zeroing them.
						let load_size = segment.load_size().saturating_sub(byte_offset).min(4096);
						let load_virt = segment.load_address() + byte_offset;
						let target_virt = segment.target_address() + byte_offset;

						let local_page_virt =
							Phys::from_address_unchecked(phys_addr).as_mut_ptr_unchecked::<u8>();

						// SAFETY(qix-): We can assume the kernel module is valid given that it's
						// SAFETY(qix-): been loaded by the bootloader.
						let (src, dest) = unsafe {
							(
								core::slice::from_raw_parts(load_virt as *const u8, load_size),
								core::slice::from_raw_parts_mut(local_page_virt, 4096),
							)
						};

						// copy data
						if load_size > 0 {
							dest[..load_size].copy_from_slice(&src[..load_size]);
						}
						// zero remaining
						if load_size < 4096 {
							dest[load_size..].fill(0);
						}

						mapper_segment
							.map_nofree(mapper, target_virt, phys_addr)
							.expect("failed to map segment");
					}
				}

				Some(elf.entry_point())
			});

			let Some(entry_point) = entry_point else {
				continue;
			};

			let instance = Instance::new(&module_handle, root_ring)
				.expect("failed to create root ring instance");

			// Create a thread for the entry point.
			let thread = Thread::new(&instance, entry_point)
				.expect("failed to create root ring instance thread");

			// Spawn it.
			Thread::spawn(thread);
		}
	}
}

/// Initializes a secondary core.
///
/// # Safety
/// Must be called EXACTLY ONCE before [`boot()`] is called,
/// and only AFTER the initial primary core has been initialized.
pub unsafe fn initialize_secondary(lapic: Lapic) {
	initialize_core_local(lapic);
}

/// Initializes the core local kernel.
///
/// # Safety
/// Must ONLY be called ONCE for the entire lifetime of the core.
unsafe fn initialize_core_local(lapic: Lapic) {
	#[expect(static_mut_refs)]
	crate::Kernel::initialize_for_core(
		lapic.id().into(),
		KERNEL_STATE.assume_init_ref(),
		crate::core_local::CoreHandle {
			lapic,
			gdt: UnsafeCell::new(MaybeUninit::uninit()),
			tss: UnsafeCell::new(Tss::default()),
			kernel_stack: UnsafeCell::new(0),
			kernel_irq_stack: UnsafeCell::new(0),
		},
	)
	.expect("failed to initialize kernel");
}

/// Main boot sequence for all cores for each bringup
/// (including boot, including the primary core).
///
/// # Safety
/// Must be called _exactly once_ per core, per core lifetime
/// (i.e. boot, or powerdown/subsequent bringup).
///
/// **Interrupts must be disabled upon entering this function.**
pub unsafe fn boot() -> ! {
	let kernel = crate::Kernel::get();

	let (tss_offset, gdt) =
		Gdt::<5>::new().with_sys_entry(SysEntry::for_tss(kernel.handle().tss.get()));

	assert_eq!(
		tss_offset,
		crate::gdt::TSS_GDT_OFFSET,
		"TSS offset mismatch"
	);

	{
		let gdt_raw = kernel.handle().gdt.get();
		let gdt_mut = &mut *gdt_raw;
		gdt_mut.write(gdt);
		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
		gdt_mut.assume_init_ref().install();
	}

	crate::interrupt::install_idt();
	crate::syscall::install_syscall_handler();
	crate::asm::load_tss(crate::gdt::TSS_GDT_OFFSET);

	if crate::cpuid::CpuidA07C0B::get().is_some_and(|c| c.fsgsbase()) {
		// Allow userspace applications to directly modify FS/GS registers.
		// Further, we disable (for now) the timestamp instruction outside of
		// ring 0.
		// NOTE(qix-): The TSD flag is enabled here tentatively; I need to investigate
		// NOTE(qix-): a bit more the implications of allowing it from userspace applications.
		// SAFETY: We're not modifying any critical flags here that would alter the Rust VM's
		// SAFETY: assumptions about the system state or memory layout.
		unsafe {
			crate::reg::Cr4::load()
				.with_fsgsbase(true)
				.with_tsd(true /* true = cr0 only */)
				.store();
		}
	} else {
		dbg_warn!(
			"CPUID 07:0:EBX.FSGSBASE not supported; not enabling CR4.FSGSBASE (programs may not \
			 work correctly)"
		);
	}

	dbg!("boot");

	loop {
		let switch = {
			let mut lock = kernel.scheduler().lock();
			let ctx = lock.event_idle();
			drop(lock);
			ctx
		};

		match switch {
			Switch::UserToKernel | Switch::UserResume(_, _) | Switch::UserToUser(_, _) => {
				// SAFETY(qix-): Should never happen; barring a bug, the scheduler
				// SAFETY(qix-): should never think we're executing a user thread at
				// SAFETY(qix-): this point in time.
				unreachable!();
			}
			Switch::KernelResume => {
				// Intentionally blank.
			}
			Switch::KernelToUser(user_ctx, None) => {
				let (thread_cr3_phys, thread_rsp, kernel_rsp, kernel_irq_rsp) = unsafe {
					user_ctx.with(|ctx_lock| {
						let mapper = ctx_lock.mapper();

						let cr3 = mapper.base_phys;
						let rsp = ctx_lock.handle().irq_stack_ptr;
						let kernel_rsp_ptr = kernel.handle().kernel_stack.get() as u64;
						let kernel_irq_rsp_ptr = kernel.handle().kernel_irq_stack.get() as u64;
						(*kernel.handle().tss.get())
							.rsp0
							.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);

						crate::asm::set_fs_msr(ctx_lock.handle().fsbase);
						crate::asm::set_gs_msr(ctx_lock.handle().gsbase);

						(cr3, rsp, kernel_rsp_ptr, kernel_irq_rsp_ptr)
					})
				};

				asm! {
					"call oro_x86_64_kernel_to_user",
					in("rax") thread_cr3_phys,
					in("rdx") thread_rsp,
					in("r9") kernel_irq_rsp,
					in("r10") kernel_rsp,
				}
			}
			Switch::KernelToUser(user_ctx, Some(syscall_response)) => {
				let (thread_cr3_phys, thread_rsp, kernel_rsp, kernel_irq_rsp) = unsafe {
					user_ctx.with(|ctx_lock| {
						let mapper = ctx_lock.mapper();

						let cr3 = mapper.base_phys;
						let rsp = ctx_lock.handle().irq_stack_ptr;
						let kernel_rsp_ptr = kernel.handle().kernel_stack.get() as u64;
						let kernel_irq_rsp_ptr = kernel.handle().kernel_irq_stack.get() as u64;
						(*kernel.handle().tss.get())
							.rsp0
							.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);

						crate::asm::set_fs_msr(ctx_lock.handle().fsbase);
						crate::asm::set_gs_msr(ctx_lock.handle().gsbase);

						(cr3, rsp, kernel_rsp_ptr, kernel_irq_rsp_ptr)
					})
				};

				asm! {
					"call oro_x86_64_kernel_to_user_sysret",
					in("r8") thread_cr3_phys,
					in("r10") thread_rsp,
					in("rdi") kernel_irq_rsp,
					in("rsi") kernel_rsp,
					in("rax") syscall_response.error as u64,
					in("r9") syscall_response.ret,
				}
			}
		}

		// Nothing to do. Wait for an interrupt.
		// Scheduler will have asked us to set a timer
		// if it wants to be woken up.
		let kernel_rsp_ptr = kernel.handle().kernel_stack.get() as u64;

		asm! {
			"call oro_x86_64_kernel_to_idle",
			in("r9") kernel_rsp_ptr,
		}
	}
}
