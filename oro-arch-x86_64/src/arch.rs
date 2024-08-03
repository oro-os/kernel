//! Implements the [`Arch`] architecture trait for the x86_64 architecture.

#![allow(clippy::inline_always)]

use crate::{
	mem::{address_space::AddressSpaceLayout, paging::PageTable, paging_level::PagingLevel},
	xfer::TransferToken,
};
use core::{
	arch::asm,
	fmt::{self, Write},
	ptr::from_ref,
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{
		AddressSegment, AddressSpace, PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator,
		UnmapError,
	},
	sync::UnfairCriticalSpinlock,
	Arch, PrebootConfig, PrebootPrimaryConfig,
};
use uart_16550::SerialPort;

/// The shared serial port for the system.
// NOTE(qix-): This is a temporary solution until pre-boot module loading
// NOTE(qix-): is implemented.
static SERIAL: UnfairCriticalSpinlock<X86_64, SerialPort> =
	UnfairCriticalSpinlock::new(unsafe { SerialPort::new(0x3F8) });

/// x86_64 architecture support implementation for the Oro kernel.
pub struct X86_64;

unsafe impl Arch for X86_64 {
	type AddressSpace = AddressSpaceLayout;
	type InterruptState = usize;
	type TransferToken = TransferToken;

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::X86_64;

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
		unsafe { writeln!(SERIAL.lock(), "{message}") }.unwrap();
	}

	unsafe fn prepare_master_page_tables<A, C>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		C: PrebootPrimaryConfig,
		A: PageFrameAllocate + PageFrameFree,
	{
		let translator = config.physical_address_translator();

		// Allocate and write the GDT.
		let gdt_page = alloc.allocate().expect("failed to allocate page for GDT");

		let gdt_slice =
			core::slice::from_raw_parts_mut(translator.to_virtual_addr(gdt_page) as *mut u8, 4096);
		gdt_slice.fill(0);

		crate::descriptor::write_gdt(gdt_slice);

		AddressSpaceLayout::gdt()
			.map(
				mapper,
				alloc,
				translator,
				AddressSpaceLayout::gdt().range().0,
				gdt_page,
			)
			.expect("failed to map GDT page for kernel address space");

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
				.map(mapper, alloc, translator, virt, phys)
				.expect("failed to map page for transfer stubs for kernel address space");

			// Attempt to unmap it from the current address space.
			// If it's not mapped, we can ignore the error.
			stubs
				.unmap(&current_mapper, alloc, translator, virt)
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
				.map(&current_mapper, alloc, translator, virt, phys)
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
			core_id:         config.core_id(),
			core_is_primary: matches!(config, PrebootConfig::Primary { .. }),
		}
	}

	unsafe fn transfer(
		entry: usize,
		transfer_token: Self::TransferToken,
		boot_config_virt: usize,
		pfa_head: u64,
	) -> ! {
		crate::xfer::transfer(entry, &transfer_token, boot_config_virt, pfa_head)
	}

	unsafe fn after_transfer<A, P>(
		mapper: &<<Self as Arch>::AddressSpace as AddressSpace>::SupervisorHandle,
		translator: &P,
		alloc: &mut A,
		is_primary: bool,
	) where
		A: PageFrameAllocate + PageFrameFree,
		P: PhysicalAddressTranslator,
	{
		// Unmap and reclaim anything in the lower half.
		let l4 = &mut *(translator.to_virtual_addr(mapper.base_phys) as *mut PageTable);

		for l4_idx in 0..=255 {
			let l4_entry = &mut l4[l4_idx];
			if l4_entry.present() {
				let l3 = &mut *(translator.to_virtual_addr(l4_entry.address()) as *mut PageTable);

				for l3_idx in 0..512 {
					let l3_entry = &mut l3[l3_idx];
					if l3_entry.present() {
						let l2 = &mut *(translator.to_virtual_addr(l3_entry.address())
							as *mut PageTable);

						for l2_idx in 0..512 {
							let l2_entry = &mut l2[l2_idx];
							if l2_entry.present() {
								let l1 = &mut *(translator.to_virtual_addr(l2_entry.address())
									as *mut PageTable);

								for l1_idx in 0..512 {
									let l1_entry = &mut l1[l1_idx];
									if l1_entry.present() {
										alloc.free(l1_entry.address());
									}
								}

								let _ = l1;
								alloc.free(l2_entry.address());
							}
						}

						let _ = l2;
						alloc.free(l3_entry.address());
					}
				}

				let _ = l3;
				alloc.free(l4_entry.address());
			}

			l4_entry.reset();
		}

		// Flush the TLB
		asm!(
			"mov rax, cr3",
			"mov cr3, rax",
			out("rax") _,
			options(nostack, preserves_flags)
		);
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
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
	X86_64::disable_interrupts();

	// Initialize the serial port
	// NOTE(qix-): This is an early-development-stage stop-gap solution
	// NOTE(qix-): to the logging and debugging problem. This will be
	// NOTE(qix-): replaced with a proper pre-boot module loading system
	// NOTE(qix-): in the future.
	SERIAL.lock().init();
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
/// This function MUST NOT be called from the preboot environment.
pub unsafe fn init_kernel_primary() {
	X86_64::disable_interrupts();

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
	X86_64::disable_interrupts();

	// TODO(qix-): Wait for latch barrier

	// TODO(qix-): Ensure that the CPU has page execution protection
	// TODO(qix-): enabled. Ref 3.1.7, NX bit.
}
