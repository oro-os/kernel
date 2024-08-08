#![no_std]
#![no_main]

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
pub unsafe extern "C" fn _start() -> ! {
	let mut core_id: u64;
	let mut core_is_primary_raw: u64;
	let mut boot_config_virt: usize;
	let mut pfa_head: u64;

	::oro_arch_x86_64::transfer_params!(core_id, core_is_primary_raw, boot_config_virt, pfa_head);

	let core_type = match core_is_primary_raw {
		0 => ::oro_kernel::CoreType::Secondary,
		_ => ::oro_kernel::CoreType::Primary,
	};

	let boot_config = &*(boot_config_virt as *const ::oro_common::boot::BootConfig);

	if core_type == ::oro_kernel::CoreType::Primary {
		::oro_arch_x86_64::init_kernel_primary();
	} else {
		::oro_arch_x86_64::init_kernel_secondary();
	}

	::oro_kernel::boot(&::oro_kernel::CoreConfig {
		core_id,
		core_type,
		boot_config,
		pfa_head,
	})
}
