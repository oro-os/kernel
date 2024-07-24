//! Implements the [`Arch`] architecture trait for the x86_64 architecture.

#![allow(clippy::inline_always)]

use crate::{
	mem::{address_space::AddressSpaceLayout, paging_level::PagingLevel},
	xfer::TransferToken,
};
use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
	ptr::from_ref,
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{AddressSegment, AddressSpace, PageFrameAllocate, PageFrameFree, UnmapError},
	sync::UnfairCriticalSpinlock,
	Arch, PrebootConfig, PrebootPrimaryConfig,
};
use uart_16550::SerialPort;

/// The shared serial port for the system.
///
/// **NOTE:** This is a temporary solution until pre-boot module loading
/// is implemented.
static SERIAL: UnfairCriticalSpinlock<X86_64, MaybeUninit<SerialPort>> =
	UnfairCriticalSpinlock::new(MaybeUninit::uninit());

/// x86_64 architecture support implementation for the Oro kernel.
pub struct X86_64;

unsafe impl Arch for X86_64 {
	type AddressSpace = AddressSpaceLayout;
	type InterruptState = usize;
	type TransferToken = TransferToken;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::X86_64;

	unsafe fn init_shared() {
		// Initialize the serial port
		SERIAL.lock().write(SerialPort::new(0x3F8));
	}

	unsafe fn init_local() {
		// TODO(qix-): Ensure that the CPU has page execution protection
		// TODO(qix-): enabled. Ref 3.1.7, NX bit.
	}

	#[cold]
	fn halt() -> ! {
		loop {
			unsafe {
				asm!("cli", "hlt");
			}
		}
	}

	#[inline(always)]
	fn disable_interrupts() {
		unsafe {
			asm!("cli", options(nostack, preserves_flags));
		}
	}

	#[inline(always)]
	fn fetch_interrupts() -> Self::InterruptState {
		let flags: usize;
		unsafe {
			asm!("pushfq", "pop {}", out(reg) flags, options(nostack));
		}
		flags
	}

	#[inline(always)]
	fn restore_interrupts(state: Self::InterruptState) {
		unsafe {
			asm!("push {}", "popfq", in(reg) state, options(nostack));
		}
	}

	fn log(message: fmt::Arguments) {
		// NOTE(qix-): This unsafe block MUST NOT PANIC.
		unsafe {
			let mut lock = SERIAL.lock();
			writeln!(lock.assume_init_mut(), "{message}")
		}
		.unwrap();
	}

	unsafe fn prepare_master_page_tables<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPrimaryConfig,
		A: PageFrameAllocate + PageFrameFree,
	{
		// Allocate and map in the transfer stubs
		let stubs_base = crate::xfer::target_address();

		let stub_start = from_ref(&crate::xfer::_ORO_STUBS_START) as usize;
		let stub_len = from_ref(&crate::xfer::_ORO_STUBS_LEN) as usize;

		debug_assert!(
			stub_start & 0xFFF == 0,
			"transfer stubs must be 4KiB aligned: {stub_start:016X}",
		);
		debug_assert!(
			stub_len & 0xFFF == 0,
			"transfer stubs length must be a multiple of 4KiB: {stub_len:X}",
		);
		debug_assert!(
			stub_len > 0,
			"transfer stubs must have a length greater than 0: {stub_len:X}",
		);

		let num_pages = (stub_len + 4095) >> 12;

		let source = stub_start as *const u8;
		let dest = stubs_base as *mut u8;

		let current_mapper =
			Self::AddressSpace::current_supervisor_space(config.physical_address_translator());

		for page_offset in 0..num_pages {
			let phys = alloc
				.allocate()
				.expect("failed to allocate page for transfer stubs (out of memory)");

			let virt = stubs_base + page_offset * 4096;

			let stubs = AddressSpaceLayout::stubs();

			// Map into the target kernel page tables
			stubs
				.map(
					mapper,
					alloc,
					config.physical_address_translator(),
					virt,
					phys,
				)
				.expect("failed to map page for transfer stubs for kernel address space");

			// Attempt to unmap it from the current address space.
			// If it's not mapped, we can ignore the error.
			stubs
				.unmap(
					&current_mapper,
					alloc,
					config.physical_address_translator(),
					virt,
				)
				.or_else(|e| {
					if e == UnmapError::NotMapped {
						Ok(0)
					} else {
						Err(e)
					}
				})
				.expect("failed to unmap page for transfer stubs from current address space");

			// Now map it into the current mapper so we can access it.
			stubs
				.map(
					&current_mapper,
					alloc,
					config.physical_address_translator(),
					virt,
					phys,
				)
				.expect("failed to map page for transfer stubs in current address space");
		}

		dest.copy_from(source, stub_len);

		Self::strong_memory_barrier();
	}

	#[allow(clippy::too_many_lines)]
	unsafe fn prepare_transfer<A, C>(
		mapper: <<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) -> Self::TransferToken
	where
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig,
	{
		// Allocate a stack for the kernel
		#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
		let last_stack_page_virt = match PagingLevel::current_from_cpu() {
			PagingLevel::Level4 => {
				(((((AddressSpaceLayout::KERNEL_STACK_IDX << 39) | 0x7F_FFFF_F000) << 16) as isize)
					>> 16) as usize
			}
			PagingLevel::Level5 => {
				(((((AddressSpaceLayout::KERNEL_STACK_IDX << 48) | 0xFFFF_FFFF_F000) << 7)
					as isize) >> 7) as usize
			}
		};

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
			stack_ptr:       last_stack_page_virt,
			page_table_phys: mapper.base_phys,
		}
	}

	unsafe fn transfer(entry: usize, transfer_token: Self::TransferToken) -> ! {
		crate::xfer::transfer(entry, &transfer_token)
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
	}
}
