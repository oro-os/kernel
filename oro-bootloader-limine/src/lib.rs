//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]
#![deny(missing_docs)]
#![feature(type_alias_impl_trait)]

use core::{ffi::CStr, mem::MaybeUninit};
#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{
	memory_map::EntryType,
	modules::InternalModule,
	request::{BootTimeRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, SmpRequest},
	response::SmpResponse,
	smp::Cpu,
	BaseRevision,
};
use oro_common::{
	boot::{BootConfig, BootInstanceType, BootMemoryRegion, CloneIterator},
	dbg, dbg_err, Arch, MemoryLayoutType, MemoryRegion, MemoryRegionType, PrebootConfig,
};

const KERNEL_PATH: &CStr = limine::cstr!("/oro-kernel");

#[used]
static BASE_REVISION: BaseRevision = BaseRevision::new();
#[used]
static REQ_MODULES: ModuleRequest = ModuleRequest::with_revision(1)
	.with_internal_modules(&[&InternalModule::new().with_path(KERNEL_PATH)]);
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::with_revision(0);
#[used]
static REQ_MMAP: MemoryMapRequest = MemoryMapRequest::with_revision(0);
#[used]
static REQ_TIME: BootTimeRequest = BootTimeRequest::with_revision(0);
#[used]
static mut REQ_SMP: SmpRequest = SmpRequest::with_revision(0);
#[cfg(debug_assertions)]
#[used]
static REQ_STKSZ: StackSizeRequest = StackSizeRequest::with_revision(0).with_size(64 * 1024);

type ImplMemoryIterator = impl CloneIterator<Item = BootMemoryRegion>;
type LimineBootConfig = BootConfig<ImplMemoryIterator>;
static mut BOOT_CONFIG: MaybeUninit<LimineBootConfig> = MaybeUninit::uninit();

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
pub unsafe fn init<A: Arch, C: CpuId>() -> ! {
	// We know that there is only one CPU being used
	// in the bootloader stage.
	A::init_shared();
	A::init_local();

	A::disable_interrupts();

	dbg!(A, "limine", "boot");

	let smp_response = get_response!(mut REQ_SMP, "symmetric");

	let boot_config = generate_boot_config(smp_response);
	BOOT_CONFIG.write(boot_config);
	dbg!(A, "limine", "boot configuration generated");

	// Find the primary CPU first, and error if we don't have it.
	let primary_cpu_id = C::bootstrap_cpu_id(smp_response).expect("no bootstrap CPU found");

	for cpu in smp_response.cpus_mut() {
		if C::cpu_id(cpu) != primary_cpu_id {
			dbg!(A, "limine", "booting seconary cpu: {}", C::cpu_id(cpu));
			cpu.goto_address.write(trampoline_to_init::<A, C>);
		}
	}

	// Finally, jump the bootstrap core to the kernel.
	let hhdm_response = get_response!(REQ_HHDM, "hhdm offset");

	dbg!(A, "limine", "booting primary cpu: {primary_cpu_id}");
	initialize_kernel::<A>(&PrebootConfig {
		core_id: primary_cpu_id,
		instance_type: BootInstanceType::Primary,
		#[allow(clippy::cast_possible_truncation)]
		memory_layout_type: MemoryLayoutType::LinearMapped {
			offset: hhdm_response.offset() as usize,
		},
	})
}

unsafe fn generate_boot_config(smp_response: &SmpResponse) -> LimineBootConfig {
	let module_response = get_response!(REQ_MODULES, "module listing");
	let mmap_response = get_response!(REQ_MMAP, "memory mapping");
	let _time_response = get_response!(REQ_TIME, "bios timestamp response");
	#[cfg(debug_assertions)]
	let _stksz_response = get_response!(REQ_STKSZ, "debug stack size adjustment");

	let kernel_module = module_response
		.modules()
		.iter()
		.find(|module| module.path() == KERNEL_PATH.to_bytes());

	let Some(_kernel_module) = kernel_module else {
		panic!("failed to find kernel module: {KERNEL_PATH:?}");
	};

	let memory_regions: ImplMemoryIterator = mmap_response
		.entries()
		.iter()
		.map(|entry| {
			let ty = match entry.entry_type {
				EntryType::BOOTLOADER_RECLAIMABLE | EntryType::KERNEL_AND_MODULES => {
					MemoryRegionType::Boot
				}
				EntryType::USABLE => MemoryRegionType::Usable,
				_ => MemoryRegionType::Unusable,
			};

			BootMemoryRegion {
				base: entry.base,
				length: entry.length,
				ty,
			}
			.aligned(4096)
		})
		.filter(|region| region.length() > 0);

	LimineBootConfig {
		num_instances: smp_response.cpus().len() as u64,
		memory_regions,
	}
}

/// # Safety
/// Must ONLY be called ONCE by SECONDARY cores. DO NOT CALL FROM PRIMARY.
/// Call `initialize_kernel` directly from the bootstrap (primary) core instead.
unsafe extern "C" fn trampoline_to_init<A: Arch, C: CpuId>(smp: &Cpu) -> ! {
	A::init_local();

	let hhdm_res = get_response!(REQ_HHDM, "hhdm offset");

	initialize_kernel::<A>(&PrebootConfig {
		core_id: C::cpu_id(smp),
		instance_type: BootInstanceType::Secondary,
		#[allow(clippy::cast_possible_truncation)]
		memory_layout_type: MemoryLayoutType::LinearMapped {
			offset: hhdm_res.offset() as usize,
		},
	})
}

/// # Safety
/// MUST be called EXACTLY ONCE per core.
#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn initialize_kernel<A: Arch>(preboot_config: &PrebootConfig) -> ! {
	let boot_config = BOOT_CONFIG.assume_init_ref();
	oro_common::boot_to_kernel::<A, _>(boot_config, preboot_config);
}

/// Panic handler for the Limine bootloader stage.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn panic<A: Arch>(info: &::core::panic::PanicInfo) -> ! {
	dbg_err!(A, "limine", "panic: {:?}", info);
	A::halt()
}
