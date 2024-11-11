//! Architecture / core initialization
//! routines and global state definitions.

use core::{arch::asm, cell::UnsafeCell, mem::MaybeUninit};

use oro_debug::{dbg, dbg_err, dbg_warn};
use oro_elf::{ElfSegment, ElfSegmentType};
use oro_kernel::KernelState;
use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::AddressSegment,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};
use oro_sync::Lock;

use crate::{
	gdt::{Gdt, SysEntry},
	handler::Handler,
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
pub unsafe fn initialize_primary() {
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

	// SAFETY(qix-): We know what we're doing here.
	#[expect(static_mut_refs)]
	KernelState::init(&mut KERNEL_STATE).expect("failed to create global kernel state");

	#[expect(static_mut_refs)]
	let state = KERNEL_STATE.assume_init_ref();

	// TODO(qix-): Not sure that I like that this is ELF-aware. This may get
	// TODO(qix-): refactored at some point.
	if let Some(oro_boot_protocol::modules::ModulesKind::V0(modules)) =
		crate::boot::protocol::MODULES_REQUEST.response()
	{
		let modules = core::ptr::read_volatile(modules.assume_init_ref());
		let mut next = modules.next;

		let root_ring = state.root_ring();

		'module: while next != 0 {
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

			let id = oro_id::AnyId::from_high_low(module.id_high, module.id_low);

			let Ok(id) = oro_id::Id::<{ oro_id::IdType::Module }>::try_from(id) else {
				dbg_warn!(
					"skipping module; not a valid module ID: {:?}",
					id.as_bytes()
				);
				continue;
			};

			if id.is_internal() {
				dbg_warn!("skipping module; internal module ID: {:?}", id.as_bytes());
				continue;
			}

			dbg!(
				"loading module: {id} @ {:016X} ({})",
				module.base,
				module.length
			);

			let module_handle = state
				.create_module(id.clone())
				.expect("failed to create root ring module");

			let entry_point = {
				let module_lock = module_handle.lock();

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
						ElfSegmentType::Ignored => continue 'module,
						ElfSegmentType::Invalid { flags, ptype } => {
							dbg_err!(
								"root ring module {id} has invalid segment; skipping: \
								 ptype={ptype:?} flags={flags:?}",
							);
							continue 'module;
						}
						ElfSegmentType::ModuleCode => AddressSpaceLayout::module_code(),
						ElfSegmentType::ModuleData => AddressSpaceLayout::module_data(),
						ElfSegmentType::ModuleRoData => AddressSpaceLayout::module_rodata(),
						ty => {
							dbg_err!("root ring module {id} has invalid segment {ty:?}; skipping",);
							continue 'module;
						}
					};

					dbg!(
						"{id}: loading {:?} segment: {:016X} {:016X} -> {:016X} ({})",
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

				elf.entry_point()
			};

			let instance = state
				.create_instance(module_handle, root_ring.clone())
				.expect("failed to create root ring instance");

			// Create a thread for the entry point.
			// TODO(qix-): Allow stack size to be passed in via module command line.
			let _thread = state
				.create_thread(
					instance,
					16 * 1024,
					crate::ThreadState::new(u64::try_from(entry_point).unwrap()),
				)
				.expect("failed to create root ring instance thread");
		}
	}
}

/// Main boot sequence for all cores for each bringup
/// (including boot, including the primary core).
///
/// # Safety
/// Must be called _exactly once_ per core, per core lifetime
/// (i.e. boot, or powerdown/subsequent bringup).
///
/// **Interrupts must be disabled upon entering this function.**
pub unsafe fn boot(lapic: Lapic) -> ! {
	// SAFETY(qix-): THIS MUST ABSOLUTELY BE FIRST.
	#[expect(static_mut_refs)]
	let kernel = crate::Kernel::initialize_for_core(
		lapic.id().into(),
		KERNEL_STATE.assume_init_ref(),
		crate::CoreState {
			lapic,
			gdt: UnsafeCell::new(MaybeUninit::uninit()),
			tss: UnsafeCell::new(Tss::default()),
			kernel_stack: UnsafeCell::new(0),
			kernel_irq_stack: UnsafeCell::new(0),
		},
	)
	.expect("failed to initialize kernel");

	let (tss_offset, gdt) =
		Gdt::<5>::new().with_sys_entry(SysEntry::for_tss(kernel.core().tss.get()));

	assert_eq!(tss_offset, crate::TSS_GDT_OFFSET, "TSS offset mismatch");

	{
		let gdt_raw = kernel.core().gdt.get();
		let gdt_mut = &mut *gdt_raw;
		gdt_mut.write(gdt);
		core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
		gdt_mut.assume_init_ref().install();
	}

	crate::interrupt::install_idt();
	crate::asm::load_tss(crate::TSS_GDT_OFFSET);

	dbg!("boot");

	let handler = Handler::new();
	loop {
		let maybe_ctx = {
			let mut lock = handler.kernel().scheduler().lock();
			let ctx = lock.event_idle(&handler);
			drop(lock);
			ctx
		};

		if let Some(user_ctx) = maybe_ctx {
			let (thread_cr3_phys, thread_rsp, kernel_rsp, kernel_irq_rsp) = unsafe {
				let ctx_lock = user_ctx.lock();
				let cr3 = ctx_lock.mapper().base_phys;
				let rsp = ctx_lock.thread_state().irq_stack_ptr;
				let kernel_rsp_ptr = kernel.core().kernel_stack.get() as u64;
				let kernel_irq_rsp_ptr = kernel.core().kernel_irq_stack.get() as u64;
				(*kernel.core().tss.get())
					.rsp0
					.write(AddressSpaceLayout::interrupt_stack().range().1 as u64 & !0xFFF);
				drop(ctx_lock);
				(cr3, rsp, kernel_rsp_ptr, kernel_irq_rsp_ptr)
			};

			asm! {
				"call oro_x86_64_kernel_to_user",
				in("rax") thread_cr3_phys,
				in("rdx") thread_rsp,
				in("r9") kernel_irq_rsp,
				in("r10") kernel_rsp,
			}
		} else {
			// Nothing to do. Wait for an interrupt.
			// Scheduler will have asked us to set a timer
			// if it wants to be woken up.
			let kernel_rsp_ptr = kernel.core().kernel_stack.get() as u64;

			asm! {
				"call oro_x86_64_kernel_to_idle",
				in("r9") kernel_rsp_ptr,
			}
		}
	}
}
