//! Initialization routine. See [`boot_to_kernel`] for more information.

use oro_arch::Target;
use oro_common::{
	arch::Arch,
	dbg, dbg_warn,
	mem::{
		mapper::{AddressSegment, AddressSpace},
		pfa::{
			alloc::{PageFrameAllocate, PageFrameFree},
			filo::FiloPageFrameAllocator,
		},
		region::{MemoryRegion, MemoryRegionType},
		translate::PhysicalAddressTranslator,
	},
	preboot::{PrebootConfig, PrebootPrimaryConfig},
	sync::{barrier::SpinBarrier, spinlock::unfair::UnfairSpinlock},
	util::erased::Erased,
};
use oro_common_elf::{ElfSegment, ElfSegmentType};

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
/// This function is probably the most heavily specified function in the entire
/// kernel. It is responsible for booting the kernel from the pre-boot environment
/// and must be called with care. It is _heavily_ environment-sensitive and a number
/// of invariants must be true prior to calling this function.
///
/// This documentation also includes some specifications for how the initialization
/// routine will behave that callers should be aware of (and as such, to which
/// maintainers must adhere).
///
/// Read the following sections **carefully** before calling this function.
///
/// Further, for **each architecture** the preboot stage supports, you must also
/// check any architecture-specific safety requirements in the architecture's
/// crate documentation (e.g. `oro-arch-x86_64`) **carefully** before calling this
/// function.
///
/// ## SMP Invocations
/// This function must be called **exactly once** per initialized core in the system.
/// If this function is not called on a core, then the kernel will have absolutely no
/// knowledge of that core's existence; the operating system will simply not report it,
/// nor will it contribute to the core count. The user _will not_ be able to use the core.
///
/// ### Timing
/// All cores must be initialized at the same stage of the pre-boot process; that is,
/// no CPU-wide alterations to any of its state may be made after the first invocation
/// to this function (on at least one of the cores).
///
/// ### Core Count
/// The number of times this function is called **must** match the count provided
/// to [`PrebootConfig::Primary::num_instances`]. If this count is not correct, the initialization
/// sequence will hang indefinitely due to the use of barriers.
///
/// ### Core ID
/// The core ID specified in the `core_id` fields of the [`PrebootConfig`] enum **must**
/// be unique between all cores in the system. Not doing so will invoke undefined, potentially
/// catastrophic behavior.
///
/// Core IDs do not have to be in any particular order. Note that strangely numbered cores
/// will pose a strange user experience if they don't map to some kind of logical core
/// numbering. They do not have to be contiguous nor correspond to the core count, and _should_
/// map to some architecture-specific, meaningful ID if sequential ordering is not used
/// or desirable.
///
/// ## Memory
/// There are a number of important memory considerations that must be taken into account
/// before calling this function.
///
/// ### Direct Mapping
/// The kernel expects a **direct mapping** of **all** physical memory
/// available to the system (or, at least, reported by the pre-boot environment)
/// in such a way that a virtual address can be derived from a physical page address.
///
/// This mapping _does not_ need to be a linear mapping, so long as a unique, non-overlapping
/// virtual address can be derived from a physical address in a 1:1 fashion.
///
/// Typically, a bootloader would set up an offset-based ("linear") physical-to-virtual
/// address mapping, but this is not a requirement. A [`oro_common::mem::translate::PhysicalAddressTranslator`]
/// implementation is all that is necessary.
///
/// The translation of physical addresses to virtual addresses **must be consistent** across
/// all cores. This includes both the mechanism by which the translation is performed, as well as
/// any offsets or other configuration values to the translator.
///
/// Put another way, the same physical address must always map to the same virtual address across
/// all translations, across all cores.
///
/// ### Stack Memory
/// All cores **must** share the same direct memory map described above, with the exception of
/// stack memory. As long as it doesn't conflict with the direct memory map, the stack pointer
/// (or whatever equivalent mechanism for the target architecture) may point to "private" page
/// mappings not shared between cores.
///
/// ### Bad Memory
/// If the pre-boot environment is capable of detecting and reporting "bad" regions of memory,
/// then the [`MemoryRegionType::Bad`] region can be reported by the memory map iterator.
/// Even if no bad memory is encountered, the [`PrebootPrimaryConfig::BAD_MEMORY_REPORTED`] field
/// should be set to `true` if the environment is _capable_ of reporting bad memory.
///
/// In the event that bad memory is reported by the aforementioned configuration field is `false`,
/// the memory will be treated and counted as "unusable" memory, which is undesirable for the
/// user.
///
/// Pre-boot environments that skip over, or otherwise do not report bad memory should set
/// the flag to `false` and refrain from producing `MemoryRegionType::Bad` variants for memory
/// regions.
///
/// ## ABI
/// The ABI of this function is strictly defined (aside from it using the Rust ABI).
///
/// ### Type Coherence
/// The types provided to this function by way of [`PrebootPrimaryConfig`] must be **identical**
/// across all core invocations. This includes both types and configuration values.
///
/// ### Linkage
/// This function **must not** be invoked across a linker boundary.
///
/// Simply put, this function must be called from within the same binary that it is defined in.
/// This means that e.g. bootloader crates **must** consume `oro-common` directly and not
/// through a separate crate/module/shared library that links to `oro-common` or otherwise
/// dynamically links to it.
///
/// A linker boundary can be crossed if the pre-boot environment has written adequate
/// stubs or trampolines to call this function as part of the binary that houses it.
///
/// This is due to the size and bounds checking of the generics of this function, which
/// cannot be enforced if the function is called dynamically (at runtime), especially
/// with types that differ from when it was compiled.
///
/// ## Architecture Specific Requirements
/// Please consult the documentation for the architecture-specific entry-points
/// in `oro-kernel/src/bin/*.rs` for any additional requirements or constraints
/// that may be placed on this function by specific architectures.
#[allow(
	clippy::needless_pass_by_value,
	clippy::too_many_lines,
	clippy::missing_docs_in_private_items
)]
pub unsafe fn boot_to_kernel<P>(config: PrebootConfig<P>) -> !
where
	P: PrebootPrimaryConfig,
{
	/// Waits for all cores to reach a certain point in the initialization sequence.
	macro_rules! wait_for_all_cores {
		() => {{
			static BARRIER: SpinBarrier = SpinBarrier::new();

			if let PrebootConfig::Primary { num_instances, .. } = &config {
				BARRIER.set_total::<Target>(*num_instances);
			}

			BARRIER.wait();
		}};
	}

	static mut KERNEL_ADDRESS_SPACE: Erased<256> = Erased::Uninit;
	static mut KERNEL_ENTRY_POINT: usize = 0;

	static MAPPER_DUPLICATE_BARRIER: SpinBarrier = SpinBarrier::new();
	static MAPPER_DUPLICATE_FINISH_BARRIER: SpinBarrier = SpinBarrier::new();
	static TRANSFER_BARRIER: SpinBarrier = SpinBarrier::new();

	Target::disable_interrupts();

	dbg!(
		"boot_to_kernel",
		"booting to kernel ({} core {})",
		match &config {
			PrebootConfig::Primary { .. } => "primary",
			PrebootConfig::Secondary { .. } => "secondary",
		},
		config.core_id(),
	);

	// Create the shared PFA between all cores.
	let pfa = {
		// This is an interesting yet seemingly necessary dance, needed
		// to make Rust infer all of the types of both the incoming iterator
		// as well as whatever iterator we need to create for the PFA.
		//
		// By placing this here and interacting with it once, the compiler
		// will infer its type based on the assignment down below. We then
		// immediately take the value back out, making it `None` again, but
		// the type will have stuck even outside of the Primary-only blocks
		// of code.
		//
		// We can then extract the type of the PFA (and all of its inferred
		// iterator types) by using the `as_slice()` method - one of the few
		// methods that is both safe and that can be called on `Option` without
		// itself returning another `Option` (which we wouldn't be able to unwrap
		// and keep a safe ABI). If it's `None` (it is, of course) it just returns
		// an empty slice. However, even a slice of zero elements has a type, and
		// that slice can be turned into a pointer.
		//
		// So we do that, and then assign it to a mutable variable, allowing Rust
		// to infer that variable's type (a pointer to our aligned PFA). The pointer
		// is of course not valid, but we can then use the type of the variable
		// to re-assign it with a call to `cast()` coming from the type-erased
		// SHARED_PFA value - which we otherwise would have to explicitly specify
		// a pointee type for.
		//
		// Thus, the cast goes through and results in a *valid, correctly aligned*
		// pointer to the shared PFA, which we can then turn into a perfectly
		// safe immutable reference, extract the inner PFA from it, and get a
		// shared reference to the PFA across all cores - without explicitly
		// specifying any of the iterator types, and allowing bootloaders to
		// construct elaborate iterator types so long as they fit within a page.
		//
		// All thanks to this little `None` here.
		//
		// Hours spent working on this: ~12.
		// For lore's sake, you can see an incredibly horrifying earlier
		// version of this dance at https://gist.github.com/Qix-/740eec4b23bf71d87ca1e9c428d36c3f.
		//
		// Qix-
		let mut temporary_pfa = None;

		if let PrebootConfig::Primary {
			memory_regions,
			physical_address_translator,
			..
		} = &config
		{
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

			// Wrap in a spinlock
			let shared_pfa = UnfairSpinlock::new(shared_pfa);

			// Make sure that we're not exceeding our page size.
			oro_common_assertions::fits1::<_, 4096>(&shared_pfa);

			// Store the PFA in the type inference helper then pop
			// it back out again. This is a dance to get the type
			// of the PFA so we can cast the SHARED_PFA to it later
			// on, without having to name the concrete types of every
			// iterator the bootloader/init routine uses.
			temporary_pfa = Some(shared_pfa);
			let Some(shared_pfa) = temporary_pfa.take() else {
				unreachable!();
			};

			// Finally, write it to the shared PFA.
			core::ptr::write_volatile(SHARED_PFA.0.as_mut_ptr().cast(), shared_pfa);
		}

		// Let everyone catch up.
		wait_for_all_cores!();
		Target::strong_memory_barrier();

		// Then we down-cast it back to a reference
		#[allow(unused_assignments)]
		let mut pfa = temporary_pfa.as_slice().as_ptr();
		pfa = SHARED_PFA.0.as_ptr().cast();
		&(*pfa)
	};

	// Finally, for good measure, make sure that we barrier here so we're all on the same page.
	wait_for_all_cores!();

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
	let kernel_mapper = match &config {
		PrebootConfig::Primary {
			kernel_module,
			num_instances,
			memory_regions,
			physical_address_translator,
			rsdp,
			..
		} => {
			let mut pfa = pfa.lock::<Target>();

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
				&mut *pfa,
				physical_address_translator,
			) else {
				panic!("failed to create preboot address space for kernel; out of memory");
			};

			let num_segments = kernel_elf.segments().count();
			dbg!(
				"boot_to_kernel",
				"mapping {} kernel segments...",
				num_segments
			);

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
						kernel_request_scanner =
							Some(oro_boot_protocol::util::RequestScanner::new(
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
						&mut *pfa,
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
					"boot_to_kernel",
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
					"boot_to_kernel",
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
							&mut *pfa,
							physical_address_translator,
							virt,
							phys,
						)
						.expect("failed to map direct map segment");
				}
			}

			dbg!("boot_to_kernel", "direct mapped all memory regions");

			// Allow the architecture to prepare any additional mappings.
			Target::prepare_primary_page_tables(&kernel_mapper, &config, &mut *pfa);

			dbg!(
				"boot_to_kernel",
				"architecture prepared primary page tables"
			);

			// Make each of the registry segments shared.
			Target::make_segment_shared(
				&kernel_mapper,
				&<Target as Arch>::AddressSpace::kernel_port_registry(),
				&config,
				&mut *pfa,
			);

			dbg!("boot_to_kernel", "initialized shared port registry segment");

			Target::make_segment_shared(
				&kernel_mapper,
				&<Target as Arch>::AddressSpace::kernel_module_instance_registry(),
				&config,
				&mut *pfa,
			);

			dbg!(
				"boot_to_kernel",
				"initialized shared module instance registry segment"
			);

			Target::make_segment_shared(
				&kernel_mapper,
				&<Target as Arch>::AddressSpace::kernel_ring_registry(),
				&config,
				&mut *pfa,
			);

			dbg!("boot_to_kernel", "initialized shared ring registry segment");

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

				match kernel_request.response_mut_unchecked().expect(
					"kernel settings request exists in kernel but is an unsupported revision",
				) {
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
				dbg_warn!(
					"boot_to_kernel",
					"kernel didn't request kernel settings; is this an Oro kernel?"
				);
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
						"boot_to_kernel",
						"kernel requested ACPI RSDP pointer but bootloader didn't provide one; \
						 kernel will be upset to learn about this"
					);
				}
			}

			// Store the kernel address space handle and entry point for cloning later.
			KERNEL_ADDRESS_SPACE = Erased::from(kernel_mapper);
			KERNEL_ENTRY_POINT = kernel_elf.entry_point();

			// Wait for all cores to see the write.
			Target::strong_memory_barrier();

			// Drop the PFA lock so the secondaries can use it.
			drop(pfa);

			dbg!(
				"boot_to_kernel",
				"primary core ready to duplicate kernel address space to secondaries; \
				 synchronizing..."
			);

			// Let other cores take it.
			MAPPER_DUPLICATE_BARRIER.set_total::<Target>(*num_instances);
			MAPPER_DUPLICATE_BARRIER.wait();

			dbg!(
				"boot_to_kernel",
				"secondaries duplicating kernel address space..."
			);

			// Let other core finish duplicating it.
			MAPPER_DUPLICATE_FINISH_BARRIER.set_total::<Target>(*num_instances);
			MAPPER_DUPLICATE_FINISH_BARRIER.wait();

			dbg!(
				"boot_to_kernel",
				"all cores have duplicated kernel address space"
			);

			// SAFETY: If unwrap fails, another core took the handle (a bug in this function alone).
			KERNEL_ADDRESS_SPACE.take().unwrap()
		}
		PrebootConfig::Secondary {
			physical_address_translator,
			..
		} => {
			// Wait for the primary to tell us the mapper handle is available.
			MAPPER_DUPLICATE_BARRIER.wait();

			// Clone the kernel address space token.
			// SAFETY: If unwrap fails, either another core took the handle, or the primary core
			// SAFETY: didn't properly set it up (a bug in this function alone).
			let kernel_address_space_primary_handle: &<<Target as Arch>::AddressSpace as AddressSpace>::SupervisorHandle = KERNEL_ADDRESS_SPACE.as_ref().unwrap();

			// Clone the kernel address space.
			let mut pfa = pfa.lock::<Target>();
			let mapper = <Target as Arch>::AddressSpace::duplicate_supervisor_space_shallow(
				kernel_address_space_primary_handle,
				&mut *pfa,
				physical_address_translator,
			)
			.expect("failed to duplicate kernel address space for secondary core; out of memory");

			// Let other secondaries use the PFA.
			drop(pfa);

			// Signal that we've finished duplicating it and that the primary core is now
			// free to take it back.
			MAPPER_DUPLICATE_FINISH_BARRIER.wait();

			mapper
		}
	};

	// Wait for all cores to come online
	wait_for_all_cores!();
	if let PrebootConfig::Primary { num_instances, .. } = &config {
		dbg!("boot_to_kernel", "all {} core(s) online", num_instances);
	}

	// Inform the architecture we are about to jump to the kernel.
	// This allows for any architecture-specific, **potentially destructive**
	// operations to be performed before the kernel is entered.
	// We start with the primary core, sync, and then let the secondaries
	// go.
	let transfer_token = match config {
		PrebootConfig::Primary { num_instances, .. } => {
			let mut pfa = pfa.lock::<Target>();
			let token = Target::prepare_transfer(kernel_mapper, &config, &mut *pfa);
			drop(pfa);

			// Inform secondaries they can now prepare for transfer
			TRANSFER_BARRIER.set_total::<Target>(num_instances);
			TRANSFER_BARRIER.wait();

			token
		}
		PrebootConfig::Secondary { .. } => {
			// Wait for primary to finish preparing for transfer
			TRANSFER_BARRIER.wait();

			let mut pfa = pfa.lock::<Target>();
			Target::prepare_transfer(kernel_mapper, &config, &mut *pfa)
		}
	};

	// Wait for all cores to be ready to jump to the kernel.
	// We do this here since allocations may fail, cores may panic, etc.
	wait_for_all_cores!();

	// XXX TODO(qix-): temporary workaround during the boot sequence refactor.
	let pfa_head = {
		let last_free = pfa.lock::<Target>().last_free();
		// SAFETY(qix-): We do this here to prevent any further usage of the PFA prior to transfer.
		let _ = pfa;
		last_free
	};

	// XXX TODO(qix-): Note that this is a temporary workaround. It effectively makes it
	// XXX TODO(qix-): impossible to boot more than one core at the moment.
	if matches!(config, PrebootConfig::Primary { .. }) {
		#[allow(clippy::enum_glob_use)]
		use oro_boot_protocol::pfa_head::PfaHeadKindMut::*;

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
						"pfa head request exists in the kernel but the initialization routine \
						 doesn't support revision {}",
						pfa_request.header.revision
					)
				}
			}
		} else {
			dbg_warn!(
				"boot_to_kernel",
				"kernel didn't request PFA head; is this an Oro kernel?"
			);
		}
	}

	// Finally, jump to the kernel entry point.
	Target::transfer(KERNEL_ENTRY_POINT, transfer_token)
}

/// A page-aligned page of bytes.
#[repr(C, align(4096))]
struct AlignedPageBytes([u8; 4096]);

/// Where the shared PFA lives; this is the memory referred to
/// by each of the cores, but downcasted as the PFA itself.
#[used]
static mut SHARED_PFA: AlignedPageBytes = AlignedPageBytes([0; 4096]);
/// Where the shared virtual address of the boot config lives.
#[used]
static mut SHARED_BOOT_CONFIG_VIRT: usize = 0;
