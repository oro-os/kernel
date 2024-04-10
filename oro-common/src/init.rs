//! Initialization sequence for the Oro kernel, including associated
//! configuration types.
//!
//! The role of a bootloader implementation in Oro is to ultimately
//! call this `init()` function with a proper configuration, which
//! provides a clean and standardized way of initializing and booting
//! into the Oro kernel without needing to know the specifics of the
//! kernel's initialization process.
//!
//! There are a _lot_ of safety requirements for running the initialization
//! sequence; please read _and understand_ the documentation for the
//! [`boot_to_kernel`] function before calling it.
use crate::{
	dbg,
	elf::{ElfSegment, ElfSegmentType},
	mem::{
		CloneToken, MemoryRegion, MemoryRegionType, MmapPageFrameAllocator, PageFrameAllocate,
		PanicOnFreeAllocator, PhysicalAddressTranslator, PrebootAddressSpace,
		SupervisorAddressSegment, SupervisorAddressSpace,
	},
	sync::{SpinBarrier, UnfairSpinlock},
	Arch,
};
use core::mem::MaybeUninit;

/// Waits for all cores to reach a certain point in the initialization sequence.
macro_rules! wait_for_all_cores {
	($config:expr) => {{
		static BARRIER: SpinBarrier = SpinBarrier::new();

		if let PrebootConfig::Primary { num_instances, .. } = &$config {
			BARRIER.set_total::<A>(*num_instances);
		}

		BARRIER.wait();
	}};
}

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
/// ## SMP Invocations
/// This function must be called **exactly once** per initialized core in the system.
/// If this function is not called on a core, then the kernel will have absolutely no
/// knowledge of that core's existence; the operating system will simply not report it,
/// nor will it contribute to the core count. The user _will not_ be able to use the core.
///
/// All cores must be initialized at the same stage of the pre-boot process; that is,
/// no CPU-wide altercations to any of its state may be made after the first invocation
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
/// Typically, a bootloader would set up an offset-based physical-to-virtual address mapping,
/// but this is not a requirement. A [`crate::mem::PhysicalAddressTranslator`] implementation
/// is all that is necessary.
///
/// The translation of physical addresses to virtual addresses **must be consistent** across
/// all cores. This includes both the mechanism by which the translation is performed, as well as
/// any offsets or other configuration values to the translator.
///
/// Put another way, the same physical address must always map to the same virtual address across
/// all cores.
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
pub unsafe fn boot_to_kernel<A, P>(config: PrebootConfig<P>) -> !
where
	A: Arch,
	P: PrebootPrimaryConfig,
{
	static mut KERNEL_ADDRESS_SPACE_TOKEN: MaybeUninit<CloneTokenProxy> = MaybeUninit::uninit();
	static mut KERNEL_ENTRY_POINT: usize = 0;

	static MAPPER_BARRIER: SpinBarrier = SpinBarrier::new();

	A::disable_interrupts();

	dbg!(
		A,
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

		if let PrebootConfig::Primary { memory_regions, .. } = &config {
			// First, we create an iterator over the memory regions
			// that injects the region's index into an IndexedMemoryRegion.
			// This allows us to track the index of the region in the original
			// memory region list for use after all cores have initialized their
			// structures, such that we can reconcile the allocated pages against
			// the original iterator and mark the boot structures as allocated for
			// the kernel to pick up and use later on. This prevents double-use
			// of those page frames without inaccurately reporting total memory size
			// (i.e. omitting the allocated memory from total / used memory counts).
			let iterator = memory_regions
				.clone()
				.enumerate()
				.map(|(index, region)| {
					IndexedMemoryRegion {
						index,
						base: region.base(),
						size: region.length(),
						region_type: region.region_type(),
					}
				})
				.filter(is_usable_region);

			// Then, we create a memory-map PFA from the iterator and stick it
			// within a spinlock for safe access by all cores. This allows well-defined
			// future access, at least to the primary core (for now), to the PFA.
			// This allocator is page-aligned and is checked to ensure it does not
			// exceed a page in size.
			//
			// The spinlocked PFA is stored inside of a page aligned structure to ensure
			// alignment requirements are met with the type-erased `SHARED_PFA`.
			let shared_pfa = MmapPageFrameAllocator::<A, IndexedMemoryRegion, _>::new(iterator);
			let shared_pfa = PanicOnFreeAllocator(shared_pfa);
			let shared_pfa = UnfairSpinlock::new(shared_pfa);
			let shared_pfa = AlignedPfa(shared_pfa);

			// Make sure that we're not exceeding our page size.
			shared_pfa.assert_fits_in_page();

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
		wait_for_all_cores!(config);
		A::strong_memory_barrier();

		// Then we down-cast it back to a reference
		#[allow(unused_assignments)]
		let mut pfa = temporary_pfa.as_slice().as_ptr();
		pfa = SHARED_PFA.0.as_ptr().cast();
		&(*pfa).0
	};

	// Finally, for good measure, make sure that we barrier here so we're all on the same page.
	wait_for_all_cores!(config);

	let kernel_mapper = if let PrebootConfig::Primary {
		kernel_module,
		num_instances,
		..
	} = &config
	{
		let mut pfa = pfa.lock();

		// Parse the kernel ELF module.
		let kernel_elf = match crate::elf::Elf::parse::<A>(kernel_module.base, kernel_module.length)
		{
			Ok(elf) => elf,
			Err(e) => {
				panic!("failed to parse kernel ELF: {:?}", e);
			}
		};

		// Create a new preboot page table mapper for the kernel.
		// This will ultimately be cloned and used by all cores.
		let Some(kernel_mapper) =
			A::PrebootAddressSpace::new(&mut *pfa, config.physical_address_translator().clone())
		else {
			panic!("failed to create preboot address space for kernel; out of memory");
		};

		for segment in kernel_elf.segments() {
			let mut mapper_segment = match segment.ty() {
				ElfSegmentType::Ignored => continue,
				ElfSegmentType::Invalid { flags, ptype } => {
					panic!(
						"invalid segment type for kernel ELF: flags={:#X}, type={:#X}",
						flags, ptype
					);
				}
				ElfSegmentType::KernelCode => kernel_mapper.kernel_code(),
				ElfSegmentType::KernelData => kernel_mapper.kernel_data(),
				ElfSegmentType::KernelRoData => kernel_mapper.kernel_rodata(),
			};

			// NOTE(qix-): This will almost definitely be improved in the future.
			// NOTE(qix-): At the very least, hugepages will change this.
			// NOTE(qix-): There will probably be some better machinery for
			// NOTE(qix-): mapping ranges of memory in the future.
			for page in 0..(segment.target_size().saturating_add(0xFFF) >> 12) {
				let Some(phys_addr) = pfa.allocate() else {
					panic!("failed to allocate page for kernel segment: out of memory");
				};

				let byte_offset = page << 12;
				let load_size = (segment.load_size() - byte_offset).min(4096);
				let load_virt = segment.load_address() + byte_offset;
				let target_virt = segment.target_address() + byte_offset;

				let local_page_virt = config
					.physical_address_translator()
					.to_virtual_addr(phys_addr);

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

				if let Err(err) = mapper_segment.map(&mut *pfa, target_virt, phys_addr) {
					panic!(
						"failed to map kernel segment: {err:?}: ls={load_size} p={page} \
						 po={page:X?} lv={load_virt:#016X} tv={target_virt:#016X} \
						 s={segment:016X?}"
					);
				}
			}

			dbg!(
				A,
				"boot_to_kernel",
				"mapped kernel segment: {:#016X?} <{:X?}> -> {:?} <{:X?}>",
				segment.target_address(),
				segment.target_size(),
				segment.ty(),
				segment.target_size(),
			);
		}

		// Store the kernel address space token and entry point for cloning later.
		let clone_token = kernel_mapper.clone_token();
		KERNEL_ADDRESS_SPACE_TOKEN.write(CloneTokenProxy::from_token(clone_token));
		KERNEL_ENTRY_POINT = kernel_elf.entry_point();

		// Wait for all cores to see the write.
		A::strong_memory_barrier();

		// Let other cores take it.
		MAPPER_BARRIER.set_total::<A>(*num_instances);
		MAPPER_BARRIER.wait();

		kernel_mapper
	} else {
		// Wait for the primary to tell us the mapper token
		// is available.
		MAPPER_BARRIER.wait();

		// Clone the kernel address space token.
		let kernel_address_space_token = KERNEL_ADDRESS_SPACE_TOKEN.assume_init_ref().as_token();

		// Clone the kernel address space.
		let mut pfa = pfa.lock();
		A::PrebootAddressSpace::from_token(kernel_address_space_token, &mut *pfa)
	};

	// Wait for all cores to come online
	wait_for_all_cores!(config);
	if let PrebootConfig::Primary { num_instances, .. } = &config {
		dbg!(A, "boot_to_kernel", "all {} core(s) online", num_instances);
	}

	// Inform the architecture we are about to jump to the kernel.
	// This allows for any architecture-specific, **potentially destructive**
	// operations to be performed before the kernel is entered.
	{
		let mut pfa = pfa.lock();
		A::prepare_transfer(&kernel_mapper, &mut *pfa);
	}

	// Finally, jump to the kernel entry point.
	A::transfer(KERNEL_ENTRY_POINT, kernel_mapper.transfer_token())
}

/// Provides the types used by the primary core configuration values
/// specified in [`PrebootConfig`].
pub trait PrebootPrimaryConfig {
	/// The type of memory region provided by the pre-boot environment.
	type MemoryRegion: MemoryRegion + Sized + 'static;

	/// The type of memory region iterator provided by the pre-boot environment.
	type MemoryRegionIterator: Iterator<Item = Self::MemoryRegion> + Clone + 'static;

	/// The type of physical-to-virtual address translator used by the pre-boot environment.
	type PhysicalAddressTranslator: PhysicalAddressTranslator + Clone + Sized + 'static;

	/// Whether or not "bad" memory regions are reported by the pre-boot environment.
	const BAD_MEMORY_REPORTED: bool;
}

/// Provides the initialization routine with configuration information for
/// each of the cores.
///
/// # Safety
/// See [`boot_to_kernel`] for information regarding the safe use of this enum.
pub enum PrebootConfig<P>
where
	P: PrebootPrimaryConfig,
{
	/// The primary core configuration
	Primary {
		/// The **unique** core ID
		core_id: u64,
		/// The number of instances that are being booted
		num_instances: u64,
		/// An iterator over all memory regions available to the system
		memory_regions: P::MemoryRegionIterator,
		/// The physical-to-virtual address translator for the core
		physical_address_translator: P::PhysicalAddressTranslator,
		/// The module definition for the Oro kernel itself.
		kernel_module: ModuleDef,
	},
	/// A secondary core configuration
	Secondary {
		/// The **unique** core ID
		core_id: u64,
		/// The physical-to-virtual address translator for the core
		physical_address_translator: P::PhysicalAddressTranslator,
	},
}

impl<P> PrebootConfig<P>
where
	P: PrebootPrimaryConfig,
{
	/// Returns the core ID of the configuration.
	pub fn core_id(&self) -> u64 {
		match self {
			PrebootConfig::Primary { core_id, .. } | PrebootConfig::Secondary { core_id, .. } => {
				*core_id
			}
		}
	}

	/// Returns a reference to the physical-to-virtual address translator for the core.
	pub fn physical_address_translator(&self) -> &P::PhysicalAddressTranslator {
		match self {
			PrebootConfig::Primary {
				physical_address_translator,
				..
			}
			| PrebootConfig::Secondary {
				physical_address_translator,
				..
			} => physical_address_translator,
		}
	}
}

/// A module definition, providing base locations, lengths, and
/// per-module initialization configuration for both the kernel
/// and root-ring modules.
///
/// Modules must be ELF files (see the [`crate::elf`] module for
/// more information on what constitutes an ELF file valid for
/// the Oro operating system).
#[derive(Clone, Copy, Debug)]
pub struct ModuleDef {
	/// The base address of the module.
	/// **MUST** be available in the pre-boot address space.
	/// **MUST** be aligned to a 4-byte boundary.
	pub base:   usize,
	/// The length of the module in bytes.
	pub length: u64,
}

/// See usage in `boot_to_kernel` for information about this structure.
struct IndexedMemoryRegion {
	/// The index of the memory region in the original memory region list.
	/// This is **before** any filtering of the memory regions.
	index:       usize,
	/// The base address of the memory region.
	base:        u64,
	/// The size of the memory region, in bytes.
	size:        u64,
	/// The type of the memory region.
	region_type: MemoryRegionType,
}

impl MemoryRegion for IndexedMemoryRegion {
	#[cold]
	fn new_with(&self, base: u64, length: u64) -> Self {
		IndexedMemoryRegion {
			index: self.index,
			base,
			size: length,
			region_type: self.region_type,
		}
	}

	#[inline]
	fn base(&self) -> u64 {
		self.base
	}

	#[inline]
	fn length(&self) -> u64 {
		self.size
	}

	#[inline]
	fn region_type(&self) -> MemoryRegionType {
		self.region_type
	}
}

/// Determines if a memory region is usable by the pre-boot environment.
/// This is pulled out into its own function to ensure that the logic
/// is consistent when reconciling used PFA regions against the original
/// memory region iterator.
#[inline]
fn is_usable_region<R: MemoryRegion>(region: &R) -> bool {
	region.region_type() == MemoryRegionType::Usable
}

// Create a preboot address space for the kernel and map it.
#[repr(C, align(16))]

/// An opaque proxy type for [`PrebootAddressSpace::CloneToken`] types.
struct CloneTokenProxy([u8; 256]);

impl CloneTokenProxy {
	/// Creates a proxy token from a concrete token type.
	fn from_token<T: CloneToken>(token: T) -> Self {
		let mut bytes = [0; 256];
		token.assert_size_and_alignment();
		unsafe {
			let ptr = bytes.as_mut_ptr().cast::<T>();
			ptr.write(token);
		}
		Self(bytes)
	}

	/// Creates a concrete token from a proxy.
	///
	/// # Safety
	/// Must be the same type passed to `from_token`.
	/// This cannot be enforced at compile time due to
	/// const generic limitations in Rust.
	unsafe fn as_token<T: CloneToken>(&self) -> T {
		let mut maybe_token = MaybeUninit::<T>::uninit();
		maybe_token
			.as_mut_ptr()
			.copy_from_nonoverlapping(self.0.as_ptr().cast(), 1);
		maybe_token.assume_init()
	}
}

/// A page-aligned page of bytes.
#[repr(C, align(4096))]
struct AlignedPageBytes([u8; 4096]);

/// Where the shared PFA lives; this is the memory referred to
/// by each of the cores, but downcasted as the PFA itself.
#[used]
static mut SHARED_PFA: AlignedPageBytes = AlignedPageBytes([0; 4096]);

/// A page-aligned page frame allocator wrapper.
#[repr(C, align(4096))]
struct AlignedPfa<A, I>(
	UnfairSpinlock<A, PanicOnFreeAllocator<MmapPageFrameAllocator<A, IndexedMemoryRegion, I>>>,
)
where
	A: Arch,
	I: Iterator<Item = IndexedMemoryRegion> + 'static;

/// Credit to @y21 for the elegant solution to compile-time size assertions.
trait AssertFitsInPage: Sized {
	/// Performs a compile-time assertion that the size of the implementing type
	/// is less than or equal to a page size upon access. Typically not accessed
	/// directly, but instead via the [`assert_fits_in_page`] method.
	const ASSERT: () = assert!(
		core::mem::size_of::<Self>() <= 4096,
		"the PFA does not fit in a 4KiB page; reduce the size of your memory map iterator \
		 structure"
	);

	/// Performs the compile-time assertion. Can be called from non-const
	/// contexts (results in a no-op), as the assertion is performed at compile-time.
	fn assert_fits_in_page(&self) {
		() = Self::ASSERT;
	}
}

impl<A, I> const AssertFitsInPage for AlignedPfa<A, I>
where
	A: Arch,
	I: Iterator<Item = IndexedMemoryRegion> + 'static,
{
}
