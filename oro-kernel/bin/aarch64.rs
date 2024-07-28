#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;

#[inline(never)]
#[cold]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_kernel::panic::<Aarch64>(info)
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
pub unsafe extern "C" fn _start() -> ! {
	let mut core_id: u64;
	let mut core_is_primary_raw: u64;

	::oro_arch_aarch64::transfer_params!(core_id, core_is_primary_raw);

	::oro_kernel::boot::<Aarch64>(&::oro_kernel::CoreConfig {
		core_id,
		core_type: match core_is_primary_raw {
			0 => ::oro_kernel::CoreType::Secondary,
			_ => ::oro_kernel::CoreType::Primary,
		},
	})
}
