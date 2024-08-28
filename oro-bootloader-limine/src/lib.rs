//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]
#![feature(type_alias_impl_trait)]

use core::ffi::CStr;
#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{
	memory_map::EntryType,
	modules::InternalModule,
	request::{BootTimeRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, RsdpRequest},
	BaseRevision,
};
use oro_boot::{
	Arch, MemoryRegion, MemoryRegionType, ModuleDef, OffsetPhysicalAddressTranslator,
	PrebootConfig, PrebootPlatformConfig, Target,
};
use oro_debug::{dbg, dbg_err, dbg_warn};

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

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
///
/// # Panics
/// Panics if required responses aren't populated by Limine
pub unsafe fn init() -> ! {
	#[cfg(debug_assertions)]
	oro_debug::init();
	dbg!("limine", "boot");

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

	let memory_regions = make_memory_map_iterator();

	let rsdp = if let Some(rsdp_response) = REQ_RSDP.get_response() {
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

	// Finally, jump the bootstrap core to the kernel.
	dbg!("limine", "booting primary cpu");
	oro_boot::boot_to_kernel(PrebootConfig::<LiminePrimaryConfig> {
		#[allow(clippy::cast_possible_truncation)]
		physical_address_translator: OffsetPhysicalAddressTranslator::new(
			hhdm_response.offset() as usize
		),
		memory_regions,
		kernel_module: ModuleDef {
			base:   kernel_module.addr() as usize,
			length: kernel_module.size(),
		},
		rsdp,
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

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
#[allow(unused_variables)]
pub unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	dbg_err!("limine", "panic: {:?}", info);
	Target::halt()
}

/// Provides Limine-specific types to the boot sequence for use
/// in initializing and booting the Oro kernel.
struct LiminePrimaryConfig;

impl PrebootPlatformConfig for LiminePrimaryConfig {
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
