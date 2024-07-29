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
	let mut boot_config_virt: u64;
	let mut pfa_head: u64;

	::oro_arch_aarch64::transfer_params!(core_id, core_is_primary_raw, boot_config_virt, pfa_head);

	::oro_kernel::boot::<Aarch64>(&::oro_kernel::CoreConfig {
		core_id,
		core_type: match core_is_primary_raw {
			0 => ::oro_kernel::CoreType::Secondary,
			_ => ::oro_kernel::CoreType::Primary,
		},
		boot_config: &*(boot_config_virt as *const ::oro_common::BootConfig),
		pfa_head,
	})
}
