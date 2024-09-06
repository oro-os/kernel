//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]

use core::ffi::CStr;
#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{
	memory_map::EntryType,
	modules::{InternalModule, ModuleFlags},
	request::{BootTimeRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, RsdpRequest},
	BaseRevision,
};
use oro_debug::{dbg, dbg_err};

/// 1MiB of memory.
#[allow(dead_code)] // TODO(qix-): Replace this with `MiB![1]` when the oro-types crate lands
const MIB1: u64 = 1024 * 1024;

/// The number of 4KiB stack pages to allocate for the kernel.
const KERNEL_STACK_PAGES: usize = 16;

/// The path to where the Oro kernel is expected.
/// The bootloader does **not** expect it to be listed
/// as a module (but it can be).
const KERNEL_PATH: &CStr = limine::cstr!("/oro-kernel");

/// The path to where the DeviceTree blob is expected,
/// if provided. The bootloader does **not** expect it to be
/// listed as a module (but it can be).
const DTB_PATH: &CStr = limine::cstr!("/oro-device-tree.dtb");

/// Provides Limine with a base revision of the protocol
/// that this "kernel" (in Limine terms) expects.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(2);

/// Requests a list of modules provided to the kernel via
/// Limine configuration.
#[used]
static REQ_MODULES: ModuleRequest = ModuleRequest::with_revision(1).with_internal_modules(&[
	&InternalModule::new()
		.with_path(KERNEL_PATH)
		.with_flags(ModuleFlags::REQUIRED),
	&InternalModule::new().with_path(DTB_PATH),
]);

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
	dbg!("bootstrapping Oro kernel with Limine bootloader");

	let hhdm_response = get_response!(REQ_HHDM, "hhdm offset");
	let hhdm_offset = hhdm_response.offset();

	(|| {
		Err({
			let bs = oro_boot::OroBootstrapper::bootstrap(
				hhdm_offset,
				KERNEL_STACK_PAGES,
				{
					use oro_boot_protocol::{MemoryMapEntry, MemoryMapEntryType};

					let mmap_response = get_response!(REQ_MMAP, "memory mapping");

					mmap_response.entries().iter().map(|region| {
						MemoryMapEntry {
							next:   0,
							base:   region.base,
							length: region.length,
							ty:     match region.entry_type {
								EntryType::USABLE | EntryType::BOOTLOADER_RECLAIMABLE => {
									MemoryMapEntryType::Usable
								}
								EntryType::KERNEL_AND_MODULES => MemoryMapEntryType::Modules,
								EntryType::BAD_MEMORY => MemoryMapEntryType::Bad,
								_ => MemoryMapEntryType::Unknown,
							},
							used:   {
								let used = if region.entry_type == EntryType::BOOTLOADER_RECLAIMABLE
								{
									region.length
								} else {
									0
								};

								// On x86/x86_64, the first 1MiB of memory is reserved and must not be used.
								// We have to set this to at least however many bytes are in the first MiB.
								#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
								let used = if region.base < MIB1 {
									let end = region.base + region.length;
									let end_mib = end.min(MIB1);
									used.max(end_mib - region.base)
								} else {
									used
								};

								used
							},
						}
					})
				},
				{
					use oro_boot_protocol::Module;

					let module_response = get_response!(REQ_MODULES, "module listing");
					let kernel_module = module_response
						.modules()
						.iter()
						.find(|module| module.path() == KERNEL_PATH.to_bytes());

					let Some(kernel_module) = kernel_module else {
						panic!("failed to find kernel module: {KERNEL_PATH:?}");
					};

					Module {
						// Expects a physical address but the Limine system gives us
						// a virtual address. We have to un-translate it.
						base:   u64::try_from(kernel_module.addr() as usize).unwrap() - hhdm_offset,
						length: kernel_module.size(),
						next:   0,
					}
				},
			)?;

			let bs = if let Some(rsdp) = REQ_RSDP.get_response() {
				bs.send(oro_boot_protocol::acpi::AcpiDataV0 {
					rsdp: rsdp.address() as u64 - hhdm_offset,
				})
			} else {
				bs
			};

			let bs = if let Some(modules) = REQ_MODULES.get_response() {
				if let Some(dtb_module) = modules
					.modules()
					.iter()
					.find(|module| module.path() == DTB_PATH.to_bytes())
				{
					bs.send(oro_boot_protocol::device_tree::DeviceTreeDataV0 {
						base:   u64::try_from(dtb_module.addr() as usize).unwrap() - hhdm_offset,
						length: dtb_module.size(),
					})
				} else {
					bs
				}
			} else {
				bs
			};

			bs.boot_to_kernel().unwrap_err()
		})
	})()
	.unwrap()
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
#[allow(unused_variables)]
pub unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	use core::arch::asm;

	dbg_err!("panic: {:?}", info);
	loop {
		#[cfg(target_arch = "x86_64")]
		asm!("hlt");
		#[cfg(target_arch = "aarch64")]
		asm!("wfi");
	}
}
