//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]
#![deny(
	missing_docs,
	clippy::integer_division,
	clippy::missing_docs_in_private_items
)]
#![feature(type_alias_impl_trait)]

use core::ffi::CStr;
#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{
	memory_map::EntryType,
	modules::InternalModule,
	request::{
		BootTimeRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, RsdpRequest, SmpRequest,
	},
	response::SmpResponse,
	smp::Cpu,
	BaseRevision,
};
use oro_boot::{
	dbg, dbg_err, dbg_warn, Arch, MemoryRegion, MemoryRegionType, ModuleDef,
	OffsetPhysicalAddressTranslator, PrebootConfig, PrebootPrimaryConfig, Target,
};

/// The path to where the Oro kernel is expected.
/// The bootloader does **not** expect it to be listed
/// as a module (but it can be).
const KERNEL_PATH: &CStr = limine::cstr!("/oro-kernel");

/// Provides Limine with a base revision of the protocol
/// that this "kernel" (in Limine terms) expects.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(2);

/// Requests a list of modules provided to the kernel via
/// Limine configuration.
#[used]
static REQ_MODULES: ModuleRequest = ModuleRequest::with_revision(1)
	.with_internal_modules(&[&InternalModule::new().with_path(KERNEL_PATH)]);

/// Requests that Limine performs a Higher Half Direct Map (HHDM)
/// of all physical memory. Provides an offset for the HHDM.
///
/// Note that the boot stage does not rely on an identity map as we
/// will overwrite certain lower-half memory mappings when implementing
/// the stubs (as prescribed by the Oro architectures Limine supports).
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::with_revision(0);

/// Requests a physical memory map from Limine.
#[used]
static REQ_MMAP: MemoryMapRequest = MemoryMapRequest::with_revision(0);

/// Requests the BIOS timestamp from Limine.
#[used]
static REQ_TIME: BootTimeRequest = BootTimeRequest::with_revision(0);

/// Requests the RSDP pointer from Limine.
#[used]
static REQ_RSDP: RsdpRequest = RsdpRequest::with_revision(0);

/// Requests that Limine initializes secondary cores and provides
/// us a way to instruct them to jump to the boot stage entry point.
///
/// Note that the mere presence of this request causes Limine to
/// bootstrap those cores.
///
/// Marked as mutable since we have to write to the `GotoAddress` field.
#[used]
static mut REQ_SMP: SmpRequest = SmpRequest::with_revision(0);

/// In debug builds, stack size is very quickly exhausted. At time
/// of writing, Limine allocates 64KiB of stack space per core, but
/// this is not enough for debug builds.
///
/// Further, since there are no stack fences or automatic stack growing
/// implemented in this stage, we must ensure there's enough stack space
/// available for the debug build to avoid a stack overflow and subsequent
/// corruption of kernel memory.
///
/// Thus, we expand the stack size here, fairly substantially.
#[cfg(debug_assertions)]
#[used]
static REQ_STKSZ: StackSizeRequest = StackSizeRequest::with_revision(0).with_size(16 * 1024 * 1024);

/// A TAIT definition that extracts the type of the memory region iterator
/// without needing to spell it out in full.
type LimineMemoryRegionIterator = impl Iterator<Item = LimineMemoryRegion> + Clone + 'static;

/// Macro to get a response from a request, panicking if it fails.
/// All request fetches must go through this macro.
macro_rules! get_response {
	($req:ident, $label:literal) => {{
		let Some(r) = $req.get_response() else {
			panic!(concat!($label, " failed"));
		};

		r
	}};

	(mut $req:ident, $label:literal) => {{
		let Some(r) = $req.get_response_mut() else {
			panic!(concat!($label, " failed"));
		};

		r
	}};
}

/// Allows the extraction of a CPU ID from the [`Cpu`] structure
/// and for the identification of the bootstrap CPU.
///
/// This must be implemented by each architecture binary.
pub trait CpuId {
	/// Extracts the CPU ID from the [`Cpu`] structure.
	///
	/// # Safety
	/// The returned ID **must** be unique for each CPU.
	unsafe fn cpu_id(smp: &Cpu) -> u64;

	/// Returns the bootstrap CPU ID. This must match the exact,
	/// unique ID returned by `cpu_id` for the bootstrap CPU.
	///
	/// If no CPU is the bootstrap CPU, this function must return `None`.
	/// The bootloader will panic in this case.
	///
	/// # Safety
	/// This function MUST return the `cpu_id` for the bootstrap CPU,
	/// and for no others.
	unsafe fn bootstrap_cpu_id(response: &SmpResponse) -> Option<u64>;
}

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// # Panics
/// Panics if a bootstrap CPU is not found.
pub unsafe fn init<C: CpuId>() -> ! {
	dbg!("limine", "boot");

	let smp_response = get_response!(mut REQ_SMP, "smp");
	let module_response = get_response!(REQ_MODULES, "module listing");
	let hhdm_response = get_response!(REQ_HHDM, "hhdm offset");
	let _time_response = get_response!(REQ_TIME, "bios timestamp response");
	#[cfg(debug_assertions)]
	let _stksz_response = get_response!(REQ_STKSZ, "debug stack size adjustment");

	let kernel_module = module_response
		.modules()
		.iter()
		.find(|module| module.path() == KERNEL_PATH.to_bytes());

	let Some(kernel_module) = kernel_module else {
		panic!("failed to find kernel module: {KERNEL_PATH:?}");
	};

	let num_instances = smp_response.cpus().len() as u64;

	let memory_regions = make_memory_map_iterator();

	let rsdp_address = if let Some(rsdp_response) = REQ_RSDP.get_response() {
		let addr = rsdp_response.address() as u64;
		let offset = hhdm_response.offset();
		if addr < offset {
			dbg_warn!(
				"limine",
				"RSDP address is below HHDM offset! ignoring RSDP (addr: {addr:#016X?}, offset: \
				 {offset:#016X?})"
			);
			None
		} else {
			Some(addr - offset)
		}
	} else {
		None
	};

	// Find the primary CPU first, and error if we don't have it.
	let primary_cpu_id = C::bootstrap_cpu_id(smp_response).expect("no bootstrap CPU found");

	for cpu in smp_response.cpus_mut() {
		if C::cpu_id(cpu) != primary_cpu_id {
			dbg!("limine", "booting seconary cpu: {}", C::cpu_id(cpu));
			cpu.goto_address.write(trampoline_to_init::<C>);
		}
	}

	// Finally, jump the bootstrap core to the kernel.
	dbg!("limine", "booting primary cpu: {primary_cpu_id}");
	initialize_kernel(PrebootConfig::<LiminePrimaryConfig>::Primary {
		core_id: primary_cpu_id,
		num_instances,
		#[allow(clippy::cast_possible_truncation)]
		physical_address_translator: OffsetPhysicalAddressTranslator::new(
			hhdm_response.offset() as usize
		),
		memory_regions,
		rsdp_address,
		kernel_module: ModuleDef {
			base:   kernel_module.addr() as usize,
			length: kernel_module.size(),
		},
	})
}

/// Creates a memory map iterator from the Limine memory map response,
/// which maps the Limine memory map types to Oro memory map types.
///
/// This is split out solely for the purpose of populating the [`LimineMemoryRegionIterator`]
/// with the implicit type of the iterator without needing to spell it out.
fn make_memory_map_iterator() -> LimineMemoryRegionIterator {
	let mmap_response = get_response!(REQ_MMAP, "memory mapping");

	mmap_response
		.entries()
		.iter()
		.map(|region| {
			LimineMemoryRegion {
				base:       region.base,
				length:     region.length,
				entry_type: match region.entry_type {
					EntryType::USABLE => MemoryRegionType::Usable,
					EntryType::BOOTLOADER_RECLAIMABLE => MemoryRegionType::Boot,
					EntryType::BAD_MEMORY => MemoryRegionType::Bad,
					_ => MemoryRegionType::Unusable,
				},
			}
		})
		.filter(|region: &LimineMemoryRegion| region.length() > 0)
}

/// # Safety
/// Must ONLY be called ONCE by SECONDARY cores. DO NOT CALL FROM PRIMARY.
/// Call `initialize_kernel` directly from the bootstrap (primary) core instead.
unsafe extern "C" fn trampoline_to_init<C: CpuId>(smp: &Cpu) -> ! {
	let hhdm_res = get_response!(REQ_HHDM, "hhdm offset");

	initialize_kernel(PrebootConfig::Secondary {
		core_id: C::cpu_id(smp),
		#[allow(clippy::cast_possible_truncation)]
		physical_address_translator: OffsetPhysicalAddressTranslator::new(
			hhdm_res.offset() as usize
		),
	})
}

/// # Safety
/// MUST be called EXACTLY ONCE per core.
unsafe fn initialize_kernel(preboot_config: PrebootConfig<LiminePrimaryConfig>) -> ! {
	oro_boot::boot_to_kernel::<_>(preboot_config);
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	dbg_err!("limine", "panic: {:?}", info);
	Target::halt()
}

/// Provides Limine-specific types to the boot sequence for use
/// in initializing and booting the Oro kernel.
struct LiminePrimaryConfig;

impl PrebootPrimaryConfig for LiminePrimaryConfig {
	type MemoryRegion = LimineMemoryRegion;
	type MemoryRegionIterator = LimineMemoryRegionIterator;
	type PhysicalAddressTranslator = OffsetPhysicalAddressTranslator;

	const BAD_MEMORY_REPORTED: bool = true;
}

/// A simple Oro-compatible memory region type; mapped to from Limine
/// memory region types by the [`make_memory_map_iterator`] function.
struct LimineMemoryRegion {
	/// The base address of the memory region.
	base:       u64,
	/// The length of the memory region.
	length:     u64,
	/// The Oro memory region type.
	entry_type: MemoryRegionType,
}

impl MemoryRegion for LimineMemoryRegion {
	#[inline]
	fn base(&self) -> u64 {
		self.base
	}

	#[inline]
	fn length(&self) -> u64 {
		self.length
	}

	#[inline]
	fn region_type(&self) -> MemoryRegionType {
		self.entry_type
	}

	fn new_with(&self, base: u64, length: u64) -> Self {
		Self {
			base,
			length,
			entry_type: self.entry_type,
		}
	}
}
