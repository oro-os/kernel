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
// NOTE(qix-): This is a temporary solution until pre-boot module loading
// NOTE(qix-): is implemented.
static SERIAL: UnfairCriticalSpinlock<Aarch64, Option<pl011::PL011>> =
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

	unsafe fn after_transfer<A>(
		_mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		_alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
	{
		// TODO(qix-)
	}
}

/// Initializes the primary core in the preboot environment.
///
/// This function MUST be called by preboot environments prior
/// to starting any initialization sequences.
///
/// It is assumed the preboot environment initializes itself on
/// a single (primary) core prior to beginning execution on other
/// cores. It is assumed that the preboot routine will properly
/// initialize other cores and/or copy over the base settings
/// of the primary core to them prior to jumping to the kernel.
///
/// Because of this, there is no `init_preboot_secondary` function.
///
/// This function *may* be reserved (i.e. do nothing) on certain
/// platforms. However, it is still necessary that the function
/// be called to be future-proof, as it may change at a later date.
///
/// # Safety
/// This function MUST be called EXACTLY once.
///
/// The kernel MUST NOT call this function.
pub unsafe fn init_preboot_primary() {
	Aarch64::disable_interrupts();

	// NOTE(qix-): This is set up specifically for QEMU.
	// NOTE(qix-): It is a stop gap measure for early-stage-development
	// NOTE(qix-): debugging and will eventually be replaced with a
	// NOTE(qix-): proper preboot module loader.
	*(SERIAL.lock()) = Some(pl011::PL011::new::<Aarch64>(
		0x900_0000,
		24_000_000,
		115_200,
		pl011::DataBits::Eight,
		pl011::StopBits::One,
		pl011::Parity::None,
	));
}

/// Initializes the primary core in the kernel.
///
/// This function *may* be reserved (i.e. do nothing) on certain
/// platforms. However, it is still necessary that the function
/// be called to be future-proof, as it may change at a later date.
///
/// # Safety
/// This function MUST be called EXACTLY once.
///
/// This function MUST only be called on the primary core.
///
/// This function MUST NOT be called by a secondary core.
///
/// This function MUST NOT be called from the preboot environment.
pub unsafe fn init_kernel_primary() {
	Aarch64::disable_interrupts();

	// TODO(qix-): Unlock the latch barrier

	init_kernel_secondary();
}

/// Initializes a seconary core in the kernel.
///
/// This function *may* be reserved (i.e. do nothing) on certain
/// platforms. However, it is still necessary that the function
/// be called to be future-proof, as it may change at a later date.
///
/// # Safety
/// This function MUST be called EXACTLY once for each secondary core.
/// If no secondary cores are present, this function MUST NOT be called.
///
/// This function MUST only be called on secondary cores.
///
/// This function MUST NOT be called from the preboot environment.
///
/// This function MAY block until `init_kernel_primary()` has completed.
pub unsafe fn init_kernel_secondary() {
	Aarch64::disable_interrupts();

	// TODO(qix-): Wait for latch barrier
}
