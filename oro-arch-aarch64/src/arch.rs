//! Main [`Arch`] implementation for the Aarch64 architecture.

#![allow(clippy::inline_always, clippy::verbose_bit_mask)]

use crate::{mem::address_space::AddressSpaceLayout, xfer::TransferToken};
use core::{
	arch::asm,
	fmt::{self, Write},
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{AddressSegment, AddressSpace, PageFrameAllocate, PageFrameFree, UnmapError},
	sync::UnfairCriticalSpinlock,
	Arch, PrebootConfig, PrebootPrimaryConfig,
};
use oro_serial_pl011 as pl011;

/// The shared serial port for the system.
///
/// **NOTE:** This is a temporary solution until pre-boot module loading
static mut SERIAL: UnfairCriticalSpinlock<Aarch64, Option<pl011::PL011>> =
	UnfairCriticalSpinlock::new(None);

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

unsafe impl Arch for Aarch64 {
	type AddressSpace = AddressSpaceLayout;
	type InterruptState = usize;
	type TransferToken = TransferToken;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

	unsafe fn init_shared() {
		// TODO(qix-): This is set up specifically for QEMU.
		// TODO(qix-): This will need to be adapted to handle
		// TODO(qix-): different UART types and a configurable
		// TODO(qix-): base address / settings in the future.
		*(SERIAL.lock()) = Some(pl011::PL011::new::<Self>(
			0x900_0000,
			24_000_000,
			115_200,
			pl011::DataBits::Eight,
			pl011::StopBits::One,
			pl011::Parity::None,
		));
	}

	unsafe fn init_local() {
		// TODO(qix-): Assert that the granule size is 4KiB for both EL1 and EL0.
	}

	#[cold]
	fn halt() -> ! {
		loop {
			unsafe {
				asm!("wfi");
			}
		}
	}

	#[inline(always)]
	fn disable_interrupts() {
		unsafe {
			asm!("msr daifset, 0xf", options(nostack, nomem, preserves_flags));
		}
	}

	#[inline(always)]
	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("mrs {}, daif", out(reg) flags, options(nostack, nomem));
		}
		flags
	}

	#[inline(always)]
	fn restore_interrupts(state: Self::InterruptState) {
		unsafe {
			asm!("msr daif, {}", in(reg) state, options(nostack, nomem));
		}
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("dsb sy", options(nostack, preserves_flags),);
		}
	}

	fn log(message: fmt::Arguments) {
		// NOTE(qix-): This unsafe block MUST NOT PANIC.
		unsafe {
			if let Some(serial) = SERIAL.lock().as_mut() {
				writeln!(serial, "{message}")
			} else {
				Ok(())
			}
		}
		.unwrap();
	}

	unsafe fn prepare_master_page_tables<A, C>(
		_mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		_config: &PrebootConfig<C>,
		_alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig,
	{
	}

	unsafe fn prepare_transfer<A, C>(
		mapper: <<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) -> Self::TransferToken
	where
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig,
	{
		// Map the stubs
		let stubs = crate::xfer::map_stubs(alloc, config.physical_address_translator())
			.expect("failed to map transfer stubs");

		// Allocate a stack for the kernel
		#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
		let last_stack_page_virt = (((((AddressSpaceLayout::KERNEL_STACK_IDX << 39) | 0x7F_FFFF_F000)
			<< 16) as isize)
			>> 16) as usize;

		// make sure top guard page is unmapped
		match AddressSpaceLayout::kernel_stack().unmap(
			&mapper,
			alloc,
			config.physical_address_translator(),
			last_stack_page_virt,
		) {
			Ok(_) => panic!("kernel top stack guard page was already mapped"),
			Err(UnmapError::NotMapped) => {}
			Err(e) => panic!("failed to test unmap of top kernel stack guard page: {e:?}"),
		}

		let stack_phys = alloc
			.allocate()
			.expect("failed to allocate page for kernel stack (out of memory)");

		AddressSpaceLayout::kernel_stack()
			.remap(
				&mapper,
				alloc,
				config.physical_address_translator(),
				last_stack_page_virt - 4096,
				stack_phys,
			)
			.expect("failed to (re)map page for kernel stack");

		// Make sure that the bottom guard page is unmapped
		match AddressSpaceLayout::kernel_stack().unmap(
			&mapper,
			alloc,
			config.physical_address_translator(),
			last_stack_page_virt - 8192,
		) {
			Ok(_) => panic!("kernel bottom stack guard page was mapped"),
			Err(UnmapError::NotMapped) => {}
			Err(e) => panic!("failed to test unmap of kernel bottom stack guard page: {e:?}"),
		}

		// Return the token that is passed to the `transfer` function.
		TransferToken {
			stack_ptr: last_stack_page_virt,
			ttbr1_page_table_phys: mapper.base_phys,
			ttbr0_page_table_phys: stubs.ttbr0_addr,
			stubs_addr: stubs.stubs_addr,
			core_id: config.core_id(),
			core_is_primary: matches!(config, PrebootConfig::Primary { .. }),
		}
	}

	unsafe fn transfer(
		entry: usize,
		transfer_token: Self::TransferToken,
		boot_config_virt: usize,
		pfa_head: u64,
	) -> ! {
		crate::xfer::transfer(entry, &transfer_token, boot_config_virt, pfa_head);
	}
}
