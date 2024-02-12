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
use limine::{modules::InternalModule, request::ModuleRequest};
use oro_common::{dbg, dbg_err, Arch};

const KERNEL_PATH: &CStr = limine::cstr!("/oro-kernel");

#[used]
static REQ_MODULES: ModuleRequest =
	ModuleRequest::new().with_internal_modules(&[&InternalModule::new().with_path(KERNEL_PATH)]);

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

	let module_response = match REQ_MODULES.get_response() {
		Some(modules) => modules,
		None => {
			dbg_err!(A, "limine", "module request failed");
			A::halt()
		}
	};

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
