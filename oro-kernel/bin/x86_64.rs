#![no_std]
#![no_main]

use oro_boot_protocol::{KernelSettingsRequest, PfaHeadRequest};

/// The general kernel settings. Applies to all cores.
///
/// Required.
#[used]
#[link_section = ".oro_boot"]
pub static KERNEL_SETTINGS: KernelSettingsRequest = KernelSettingsRequest::with_revision(0);

/// TODO(qix-): Temporary workaround during the boot sequence refactor.
#[used]
#[link_section = ".oro_boot"]
pub static PFA_REQUEST: PfaHeadRequest = PfaHeadRequest::with_revision(0);

#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_kernel::panic(info)
}

/// Main entry point for the Oro kernel. Bootloaders jump
/// to this function to start the kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It should be
/// treated as an ELF entry point and jumped to by the
/// bootloader.
#[inline(never)]
#[cold]
#[no_mangle]
#[allow(clippy::missing_panics_doc)] // XXX(qix-) DEBUG
pub unsafe extern "C" fn _start() -> ! {
	// TODO(qix-): temporary workaround during the boot sequence refactor.
	#[allow(irrefutable_let_patterns)]
	{
		::oro_kernel::config::IS_PRIMARY_CORE = true;
		::oro_kernel::config::NUM_CORES = 1;
		::oro_kernel::config::LINEAR_MAP_OFFSET =
			if let ::oro_boot_protocol::kernel_settings::KernelSettingsKind::V0(data) =
				KERNEL_SETTINGS
					.response()
					.expect("kernel settings were not populated")
			{
				usize::try_from(data.assume_init_ref().linear_map_offset).unwrap()
			} else {
				panic!("kernel settings response is not v0");
			};
		::oro_kernel::config::PFA_HEAD =
			if let ::oro_boot_protocol::pfa_head::PfaHeadKind::V0(data) = PFA_REQUEST
				.response()
				.expect("PFA request was not populated")
			{
				data.assume_init_ref().pfa_head
			} else {
				panic!("PFA request response is not v0");
			};
	}

	::oro_kernel::boot()
}
