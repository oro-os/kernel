//! Root ring initialization procedure.

use oro_debug::{dbg, dbg_err};
use oro_elf::{ElfSegment, ElfSegmentType};
use oro_kernel::{instance::Instance, module::Module, thread::Thread};
use oro_mem::{
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace},
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use super::protocol;
use crate::mem::address_space::AddressSpaceLayout;

/// Initializes the root ring for the system
///
/// # Panics
/// Panics if an error occurs initializing the root ring.
///
/// # Safety
/// The local [`crate::Kernel`] instance must be initialized.
/// No other kernels should be running at call time.
// TODO(qix-): To be refactored entirely.
pub unsafe fn initialize_root_ring() {
	let kernel = crate::Kernel::get();

	// TODO(qix-): Not sure that I like that this is ELF-aware. This may get
	// TODO(qix-): refactored at some point.
	if let Some(oro_boot_protocol::modules::ModulesKind::V0(modules)) =
		protocol::MODULES_REQUEST.response()
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
