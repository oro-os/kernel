#![no_std]
#![no_main]

use oro_arch_aarch64::Aarch64;

#[inline(never)]
#[panic_handler]
unsafe fn panic(info: &::core::panic::PanicInfo) -> ! {
	::oro_bootloader_limine::panic::<Aarch64>(info)
}

/// Main entry point for the Limine bootloader stage
/// for the Oro kernel.
///
/// # Safety
/// Do **NOT** call this function directly. It is called
/// by the Limine bootloader.
#[inline(never)]
#[cold]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
	::oro_arch_aarch64::init_preboot_primary();
	::oro_bootloader_limine::init::<Aarch64, MpidrCpuId>()
}

struct MpidrCpuId;

impl ::oro_bootloader_limine::CpuId for MpidrCpuId {
	#[inline]
	unsafe fn cpu_id(cpu: &::limine::smp::Cpu) -> u64 {
		cpu.mpidr
	}

	#[inline]
	unsafe fn bootstrap_cpu_id(response: &limine::response::SmpResponse) -> Option<u64> {
		Some(response.bsp_mpidr())
	}
}
