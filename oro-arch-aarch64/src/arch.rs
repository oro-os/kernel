//! Main [`Arch`] implementation for the Aarch64 architecture.

#![allow(clippy::inline_always, clippy::verbose_bit_mask)]

use crate::{
	mem::{address_space::AddressSpaceLayout, paging::PageTable},
	xfer::TransferToken,
};
use core::{
	arch::asm,
	fmt::{self, Write},
};
use oro_common::{
	arch::Arch,
	interrupt::InterruptHandler,
	mem::{
		mapper::{AddressSegment, AddressSpace, UnmapError},
		pfa::alloc::{PageFrameAllocate, PageFrameFree},
		translate::PhysicalAddressTranslator,
	},
	preboot::{PrebootConfig, PrebootPlatformConfig},
	sync::spinlock::unfair_critical::UnfairCriticalSpinlock,
};
use oro_common_elf::{ElfClass, ElfEndianness, ElfMachine};
use oro_serial_pl011 as pl011;

/// The number of pages to allocate for the kernel stack.
const KERNEL_STACK_PAGES: usize = 8;

/// The shared serial port for the system.
// NOTE(qix-): This is a temporary solution until pre-boot module loading
// NOTE(qix-): is implemented.
static SERIAL: UnfairCriticalSpinlock<Option<pl011::PL011>> = UnfairCriticalSpinlock::new(None);

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

unsafe impl Arch for Aarch64 {
	type AddressSpace = AddressSpaceLayout;
	type InterruptState = usize;
	type TransferToken = TransferToken;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

	fn halt_once_and_wait() {
		unsafe {
			asm!("wfi");
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
			if let Some(serial) = SERIAL.lock::<Self>().as_mut() {
				writeln!(serial, "{message}")
			} else {
				Ok(())
			}
		}
		.unwrap();
	}

	unsafe fn prepare_primary_page_tables<A, C>(
		_mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		_config: &PrebootConfig<C>,
		_alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPlatformConfig,
	{
	}

	unsafe fn make_segment_shared<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		segment: &<Self::AddressSpace as AddressSpace>::SupervisorSegment,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPlatformConfig,
		A: PageFrameAllocate + PageFrameFree,
	{
		segment
			.make_top_level_present(mapper, alloc, &config.physical_address_translator)
			.expect("failed to map shared segment");
	}

	unsafe fn initialize_interrupts<H: InterruptHandler>() {
		// TODO(qix-)
	}

	unsafe fn prepare_transfer<A, C>(
		mapper: <<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) -> Self::TransferToken
	where
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPlatformConfig,
	{
		let translator = &config.physical_address_translator;

		// Map the stubs
		let stubs =
			crate::xfer::map_stubs(alloc, translator).expect("failed to map transfer stubs");

		// Allocate a stack for the kernel
		// TODO(qix-): This will have to change when different addressing types are supported.
		let last_stack_page_virt = AddressSpaceLayout::kernel_stack().range().1 & !0xFFF;

		// make sure top guard page is unmapped
		match AddressSpaceLayout::kernel_stack().unmap(
			&mapper,
			alloc,
			translator,
			last_stack_page_virt,
		) {
			Ok(_) => panic!("kernel top stack guard page was already mapped"),
			Err(UnmapError::NotMapped) => {}
			Err(e) => panic!("failed to test unmap of top kernel stack guard page: {e:?}"),
		}

		let mut bottom_stack_page_virt = last_stack_page_virt;

		for _ in 0..KERNEL_STACK_PAGES {
			// TODO(qix-): This will have to change when different addressing types are supported.
			bottom_stack_page_virt -= 4096;

			let stack_phys = alloc
				.allocate()
				.expect("failed to allocate page for kernel stack (out of memory)");

			AddressSpaceLayout::kernel_stack()
				.remap(
					&mapper,
					alloc,
					translator,
					bottom_stack_page_virt,
					stack_phys,
				)
				.expect("failed to (re)map page for kernel stack");
		}

		// Make sure that the bottom guard page is unmapped
		match AddressSpaceLayout::kernel_stack().unmap(
			&mapper,
			alloc,
			translator,
			bottom_stack_page_virt - 4096,
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
		}
	}

	unsafe fn transfer(entry: usize, transfer_token: Self::TransferToken) -> ! {
		crate::xfer::transfer(entry, &transfer_token);
	}

	unsafe fn after_transfer<A, P>(
		_mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		translator: &P,
		alloc: &mut A,
	) where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator,
	{
		// NOTE(qix-): `_mapper` isn't useful to use because it points to TT1.
		// NOTE(qix-): We're unmapping all of TT0.
		let tt0_phys = crate::asm::load_ttbr0();
		let l0_virt = translator.to_virtual_addr(tt0_phys);
		let l0 = &mut *(l0_virt as *mut PageTable);

		// TODO(qix-): This will absolutely need to be updated when different addressing
		// TODO(qix-): types or more than 4 page table levels are supported. Even though
		// TODO(qix-): the 'official' init routine has this tightly controlled, we can't
		// TODO(qix-): guarantee that it'll never change.
		for l0_idx in 0..512 {
			let l0_entry = &mut l0[l0_idx];
			if l0_entry.valid() {
				// SAFETY(qix-): Guaranteed to be valid by the above checks.
				let l1_phys = l0_entry.address(0).unwrap();
				let l1_virt = translator.to_virtual_addr(l1_phys);
				let l1 = &mut *(l1_virt as *mut PageTable);

				for l1_idx in 0..512 {
					let l1_entry = &mut l1[l1_idx];
					if l1_entry.valid() {
						// SAFETY(qix-): Guaranteed to be valid by the above checks.
						let l2_phys = l1_entry.address(1).unwrap();
						let l2_virt = translator.to_virtual_addr(l2_phys);
						let l2 = &mut *(l2_virt as *mut PageTable);

						for l2_idx in 0..512 {
							let l2_entry = &mut l2[l2_idx];
							if l2_entry.valid() {
								// SAFETY(qix-): Guaranteed to be valid by the above checks.
								let l3_phys = l2_entry.address(2).unwrap();
								let l3_virt = translator.to_virtual_addr(l3_phys);
								let l3 = &mut *(l3_virt as *mut PageTable);

								for l3_idx in 0..512 {
									let l3_entry = &mut l3[l3_idx];
									if l3_entry.valid() {
										// SAFETY(qix-): Guaranteed to be valid by the above checks.
										let page_phys = l3_entry.address(3).unwrap();
										alloc.free(page_phys);
									}
								}

								alloc.free(l3_phys);
							}
						}

						alloc.free(l2_phys);
					}
				}

				alloc.free(l1_phys);
			}

			// Make sure other cores see the writes.
			Self::strong_memory_barrier();
		}

		alloc.free(tt0_phys);
		crate::asm::store_ttbr0(0);
	}
}

/// Aarch64-specific configuration for the Oro kernel.
pub struct Config {
	/// The **physical** address of the Device Tree Blob (DTB)
	/// that was passed to the kernel.
	///
	/// This can be a module or baked-in value, but it is
	/// required to a contiguous physical block of memory.
	pub dtb_phys: u64,
}
