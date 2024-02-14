//! [Limine](https://github.com/limine-bootloader/limine)
//! bootloader support for the
//! [Oro Operating System](https://github.com/oro-os/kernel).
//!
//! This crate has both a library (which is common between architectures)
//! and individual, architecture-specific binaries.
//! See the `bin/` directory for architecture-specific entry points.
#![no_std]
#![deny(missing_docs)]

use core::ffi::CStr;
#[cfg(debug_assertions)]
use limine::request::StackSizeRequest;
use limine::{
	modules::InternalModule,
	request::{BootTimeRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, SmpRequest},
};
use oro_common::{dbg, dbg_err, Arch};

const KERNEL_PATH: &CStr = limine::cstr!("/oro-kernel");

#[used]
static REQ_MODULES: ModuleRequest = ModuleRequest::with_revision(0)
	.with_internal_modules(&[&InternalModule::new().with_path(KERNEL_PATH)]);
#[used]
static REQ_HHDM: HhdmRequest = HhdmRequest::with_revision(0);
#[used]
static REQ_MMAP: MemoryMapRequest = MemoryMapRequest::with_revision(0);
#[used]
static REQ_TIME: BootTimeRequest = BootTimeRequest::with_revision(0);
#[used]
static REQ_SMP: SmpRequest = SmpRequest::with_revision(0);
#[cfg(debug_assertions)]
#[used]
static REQ_STKSZ: StackSizeRequest = StackSizeRequest::with_revision(0).with_size(64 * 1024);

macro_rules! get_response {
	($A:ty, $req:ident, $label:literal) => {
		match $req.get_response() {
			Some(r) => {
				dbg!($A, "limine", concat!("got ", $label));
				r
			}
			None => {
				dbg_err!($A, "limine", concat!($label, " failed"));
				<$A>::halt();
			}
		}
	};
}

/// Runs the Limine bootloader.
///
/// # Safety
/// Do **NOT** call this function directly.
/// It is only called by the architecture-specific binaries.
pub unsafe fn init<A: Arch>() -> ! {
	// We know that there is only one CPU being used
	// in the bootloader stage.
	A::init_shared();
	A::init_local();

	A::disable_interrupts();

	dbg!(A, "limine", "boot");

	let module_response = get_response!(A, REQ_MODULES, "module listing");
	let _hhdm_response = get_response!(A, REQ_HHDM, "hhdm offset");
	let _mmap_response = get_response!(A, REQ_MMAP, "memory mapping");
	let _time_response = get_response!(A, REQ_TIME, "bios timestamp response");
	let _smp_response = get_response!(A, REQ_SMP, "symmetric");
	#[cfg(debug_assertions)]
	let _stksz_response = get_response!(A, REQ_STKSZ, "debug stack size adjustment");

	let kernel_module = module_response
		.modules()
		.iter()
		.find(|module| module.path() == KERNEL_PATH.to_bytes());
	let _kernel_module = match kernel_module {
		Some(module) => module,
		None => {
			dbg_err!(A, "limine", "failed to find kernel module: {KERNEL_PATH:?}");
			A::halt()
		}
	};

	dbg!(A, "limine", "kernel module found");
	A::halt() // TODO(qix-): Temporary.
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
