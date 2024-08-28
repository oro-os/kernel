//! Initialization routine. See [`boot_to_kernel`] for more information.

use oro_arch::Target;
// XXX TODO(qix-): Note that this is a temporary workaround. It effectively makes it
// XXX TODO(qix-): impossible to boot more than one core at the moment.
#[allow(clippy::enum_glob_use)]
use oro_boot_protocol::pfa_head::PfaHeadKindMut::*;
use oro_common::{
	arch::Arch,
	mem::{
		mapper::{AddressSegment, AddressSpace},
		pfa::{
			alloc::{PageFrameAllocate, PageFrameFree},
			filo::FiloPageFrameAllocator,
		},
		region::{MemoryRegion, MemoryRegionType},
		translate::PhysicalAddressTranslator,
	},
	preboot::{PrebootConfig, PrebootPlatformConfig},
	util::erased::Erased,
};
use oro_common_elf::{ElfSegment, ElfSegmentType};
use oro_debug::{dbg, dbg_warn};

/// Initializes and transfers execution to the Oro kernel.
///
/// This function does not return. Calling this function will boot the
/// Oro operating system kernel.
///
/// # Panics
/// Will panic if there is no memory available to allocate the initial
/// kernel structures. This should be a rare case.
///
/// # Safety
/// This function used to have a bunch of documentation regarding how
/// to initialize the kernel. However, this is in flux and is no
/// longer relevant. It's now "unsafe" because it's the entry point
/// to the kernel and the kernel is inherently unsafe - this function
/// will ultimately go away and be re-spec'd at a later time.
// TODO(qix-): Revisit the docs here if this function is kept.
#[allow(
	clippy::needless_pass_by_value,
	clippy::too_many_lines,
	clippy::missing_docs_in_private_items
)]
pub unsafe fn boot_to_kernel<P>(config: PrebootConfig<P>) -> !
where
	P: PrebootPlatformConfig,
{
	static mut KERNEL_ADDRESS_SPACE: Erased<256> = Erased::Uninit;
	static mut KERNEL_ENTRY_POINT: usize = 0;

	Target::disable_interrupts();

	dbg!("booting to kernel");

	let PrebootConfig {
		memory_regions,
		physical_address_translator,
		kernel_module,
		rsdp,
	} = &config;

	// Create the shared PFA between all cores.
	let mut pfa = {
		// Then, we create a memory-map PFA from the iterator and stick it
		// within a spinlock for safe access by all cores. This allows well-defined
		// future access, at least to the primary core (for now), to the PFA.
		// This allocator is page-aligned and is checked to ensure it does not
		// exceed a page in size.
		//
		// The spinlocked PFA is stored inside of a page aligned structure to ensure
		// alignment requirements are met with the type-erased `SHARED_PFA`.
		let mut shared_pfa = FiloPageFrameAllocator::new(physical_address_translator.clone());

		// Pre-warm the shared PFA by "freeing" all usable memory regions.
		for region in memory_regions
			.clone()
			.filter(|r| r.region_type() == MemoryRegionType::Usable)
		{
			let region = region.aligned(4096);
			for page in (0..region.length()).step_by(4096) {
				shared_pfa.free(region.base() + page);
			}
		}

		// Make sure that we're not exceeding our page size.
		oro_common_assertions::fits1::<_, 4096>(&shared_pfa);

		shared_pfa
	};

	// A place to store the kernel request scanner.
	// `None` for secondary cores.
	// TODO(qix-): This is a temporary solution until the entire boot sequence is moved
	// TODO(qix-): into the kernel, during this refactor period.
	let mut kernel_request_scanner = None;

	// Next, we create the supervisor mapper. This has two steps, as the value returned
	// is going to be different for every core.
	//
	// The primary core will first create the "genesis" mapper, which gets the kernel and all
	// other 'shared' memory regions mapped into it. Anything mapped in this mapper is expected
	// never to change, and is shared across all cores.
	//
	// Then, the primary core will move the mapping handle into a static "proxy" object, which
	// is a facade over a type-agnostic byte buffer whereby immutable references can be taken
	// in a type-safe manner. This proxy is shared across all cores.
	//
	// The CPU will then signal for all secondary cores to take a reference to the handle via
	// the proxy and duplicate it. Each core then has its own handle to the same mapping (typically,
	// the architecture will create a new root-level page table upon duplication, then copy all of the
	// same root-level mappings into it).
	//
	// The primary core will then wait for all secondary cores to duplicate the mapping, then
	// take back the handle from the proxy and return it, such that the primary core is not
	// duplicating itself and thus leaking physical pages.
	let kernel_mapper = {
		// Parse the kernel ELF module.
		let kernel_elf = match oro_common_elf::Elf::parse(
			kernel_module.base,
			kernel_module.length,
			Target::ELF_ENDIANNESS,
			Target::ELF_CLASS,
			Target::ELF_MACHINE,
		) {
			Ok(elf) => elf,
			Err(e) => {
				panic!("failed to parse kernel ELF: {:?}", e);
			}
		};

		// Create a new preboot page table mapper for the kernel.
		// This will ultimately be cloned and used by all cores.
		let Some(kernel_mapper) = <Target as Arch>::AddressSpace::new_supervisor_space(
			&mut pfa,
			physical_address_translator,
		) else {
			panic!("failed to create preboot address space for kernel; out of memory");
		};

		let num_segments = kernel_elf.segments().count();
		dbg!("mapping {} kernel segments...", num_segments);

		assert!(num_segments > 0, "kernel ELF has no segments");

		for segment in kernel_elf.segments() {
			let mapper_segment = match segment.ty() {
				ElfSegmentType::Ignored => continue,
				ElfSegmentType::Invalid { flags, ptype } => {
					panic!(
						"invalid segment type for kernel ELF: flags={:#X}, type={:#X}",
						flags, ptype
					);
				}
				ElfSegmentType::KernelCode => <Target as Arch>::AddressSpace::kernel_code(),
				ElfSegmentType::KernelData => <Target as Arch>::AddressSpace::kernel_data(),
				ElfSegmentType::KernelRoData | ElfSegmentType::KernelRequests => {
					<Target as Arch>::AddressSpace::kernel_rodata()
				}
			};

			// NOTE(qix-): This will almost definitely be improved in the future.
			// NOTE(qix-): At the very least, hugepages will change this.
			// NOTE(qix-): There will probably be some better machinery for
			// NOTE(qix-): mapping ranges of memory in the future.
			for page in 0..(segment.target_size().saturating_add(0xFFF) >> 12) {
				let Some(phys_addr) = pfa.allocate() else {
					panic!("failed to allocate page for kernel segment: out of memory");
				};

				if page == 0 && segment.ty() == ElfSegmentType::KernelRequests {
					kernel_request_scanner = Some(oro_boot_protocol::util::RequestScanner::new(
						physical_address_translator
								 	// TODO(qix-): This is temporary until the entire boot sequence is moved
									// TODO(qix-): into the kernel, during this refactor period.
									.to_virtual_addr(phys_addr) as *mut u8,
						segment.target_size(),
					));
				}

				let byte_offset = page << 12;
				// Saturating sub here since the target size might exceed the file size,
				// in which case we have to keep allocating those pages and zeroing them.
				let load_size = segment.load_size().saturating_sub(byte_offset).min(4096);
				let load_virt = segment.load_address() + byte_offset;
				let target_virt = segment.target_address() + byte_offset;

				let local_page_virt = physical_address_translator.to_virtual_addr(phys_addr);

				let dest = core::slice::from_raw_parts_mut(local_page_virt as *mut u8, 4096);
				let src = core::slice::from_raw_parts(load_virt as *const u8, load_size);

				// copy data
				if load_size > 0 {
					dest[..load_size].copy_from_slice(&src[..load_size]);
				}
				// zero remaining
				if load_size < 4096 {
					dest[load_size..].fill(0);
				}

				if let Err(err) = mapper_segment.map(
					&kernel_mapper,
					&mut pfa,
					physical_address_translator,
					target_virt,
					phys_addr,
				) {
					panic!(
						"failed to map kernel segment: {err:?}: ls={load_size} p={page} \
						 po={page:X?} lv={load_virt:#016X} tv={target_virt:#016X} \
						 s={segment:016X?}"
					);
				}
			}

			dbg!(
				"mapped kernel segment: {:#016X?} <{:X?}> -> {:?} <{:X?}>",
				segment.target_address(),
				segment.target_size(),
				segment.ty(),
				segment.target_size(),
			);
		}

		// Perform the direct map of all memory
		let direct_map = <Target as Arch>::AddressSpace::direct_map();
		let (dm_start, _) = direct_map.range();
		let min_phys_addr = memory_regions.clone().map(|r| r.base()).min().unwrap();
		assert!(
			(dm_start as u64) >= min_phys_addr,
			"direct map start below minimum physical address"
		);

		for region in memory_regions.clone() {
			dbg!(
				"mapping direct map segment: {:?}: {:#016X?} <{:X?}>",
				region.region_type(),
				region.base(),
				region.length()
			);

			let region = region.aligned(4096);
			for byte_offset in (0..region.length()).step_by(4096) {
				let phys = region.base() + byte_offset;
				#[allow(clippy::cast_possible_truncation)]
				let virt = dm_start + (phys - min_phys_addr) as usize;
				direct_map
					.map(
						&kernel_mapper,
						&mut pfa,
						physical_address_translator,
						virt,
						phys,
					)
					.expect("failed to map direct map segment");
			}
		}

		dbg!("direct mapped all memory regions");

		// Allow the architecture to prepare any additional mappings.
		Target::prepare_primary_page_tables(&kernel_mapper, &config, &mut pfa);

		dbg!("architecture prepared primary page tables");

		// Make each of the registry segments shared.
		Target::make_segment_shared(
			&kernel_mapper,
			&<Target as Arch>::AddressSpace::kernel_port_registry(),
			&config,
			&mut pfa,
		);

		dbg!("initialized shared port registry segment");

		Target::make_segment_shared(
			&kernel_mapper,
			&<Target as Arch>::AddressSpace::kernel_module_instance_registry(),
			&config,
			&mut pfa,
		);

		dbg!("initialized shared module instance registry segment");

		Target::make_segment_shared(
			&kernel_mapper,
			&<Target as Arch>::AddressSpace::kernel_ring_registry(),
			&config,
			&mut pfa,
		);

		dbg!("initialized shared ring registry segment");

		// Write the boot config.
		assert!(
			usize::try_from(min_phys_addr).is_ok(),
			"minimum physical address too large"
		);

		#[allow(clippy::cast_possible_truncation)]
		let linear_map_offset = dm_start - (min_phys_addr as usize);
		if let Some(kernel_request) = kernel_request_scanner
			.as_mut()
			.expect("no kernel request scanner")
			.get::<oro_boot_protocol::KernelSettingsRequest>()
		{
			#[allow(clippy::enum_glob_use)]
			use oro_boot_protocol::kernel_settings::KernelSettingsKindMut::*;

			match kernel_request
				.response_mut_unchecked()
				.expect("kernel settings request exists in kernel but is an unsupported revision")
			{
				V0(settings) => {
					settings.write(oro_boot_protocol::kernel_settings::KernelSettingsDataV0 {
						linear_map_offset: linear_map_offset.try_into().unwrap(),
					});
					kernel_request.populated = 1;
				}
				#[allow(unreachable_patterns)]
				_ => {
					panic!(
						"kernel settings request exists in the kernel but the initialization \
						 routine doesn't support revision {}",
						kernel_request.header.revision
					)
				}
			}
		} else {
			dbg_warn!("kernel didn't request kernel settings; is this an Oro kernel?");
		}

		if let Some(kernel_request) = kernel_request_scanner
			.as_mut()
			.expect("no kernel request scanner")
			.get::<oro_boot_protocol::AcpiRequest>()
		{
			if let Some(rsdp) = rsdp {
				#[allow(clippy::enum_glob_use)]
				use oro_boot_protocol::acpi::AcpiKindMut::*;

				match kernel_request
					.response_mut_unchecked()
					.expect("ACPI request exists in kernel but is an unsupported revision")
				{
					V0(settings) => {
						settings.write(oro_boot_protocol::acpi::AcpiDataV0 { rsdp: *rsdp });
						kernel_request.populated = 1;
					}
					#[allow(unreachable_patterns)]
					_ => {
						panic!(
							"acpi request exists in the kernel but the initialization routine \
							 doesn't support revision {}",
							kernel_request.header.revision
						)
					}
				}
			} else {
				dbg_warn!(
					"kernel requested ACPI RSDP pointer but bootloader didn't provide one; kernel \
					 will be upset to learn about this"
				);
			}
		}

		// Store the kernel address space handle and entry point for cloning later.
		KERNEL_ADDRESS_SPACE = Erased::from(kernel_mapper);
		KERNEL_ENTRY_POINT = kernel_elf.entry_point();

		dbg!(
			"primary core ready to duplicate kernel address space to secondaries; synchronizing..."
		);

		dbg!("secondaries duplicating kernel address space...");

		dbg!("all cores have duplicated kernel address space");

		// SAFETY: If unwrap fails, another core took the handle (a bug in this function alone).
		KERNEL_ADDRESS_SPACE.take().unwrap()
	};

	// Inform the architecture we are about to jump to the kernel.
	// This allows for any architecture-specific, **potentially destructive**
	// operations to be performed before the kernel is entered.
	// We start with the primary core, sync, and then let the secondaries
	// go.
	let transfer_token = Target::prepare_transfer(kernel_mapper, &config, &mut pfa);

	// XXX TODO(qix-): temporary workaround during the boot sequence refactor.
	let pfa_head = {
		let last_free = pfa.last_free();
		// SAFETY(qix-): We do this here to prevent any further usage of the PFA prior to transfer.
		let _ = pfa;
		last_free
	};

	if let Some(pfa_request) = kernel_request_scanner
		.as_mut()
		.expect("no kernel request scanner")
		.get::<oro_boot_protocol::PfaHeadRequest>()
	{
		match pfa_request
			.response_mut_unchecked()
			.expect("pfa head request exists in kernel but is an unsupported revision")
		{
			V0(pfa_head_res) => {
				pfa_head_res.write(oro_boot_protocol::pfa_head::PfaHeadDataV0 { pfa_head });
				pfa_request.populated = 1;
			}
			#[allow(unreachable_patterns)]
			_ => {
				panic!(
					"pfa head request exists in the kernel but the initialization routine doesn't \
					 support revision {}",
					pfa_request.header.revision
				)
			}
		}
	} else {
		dbg_warn!("kernel didn't request PFA head; is this an Oro kernel?");
	}

	// Finally, jump to the kernel entry point.
	Target::transfer(KERNEL_ENTRY_POINT, transfer_token)
}
