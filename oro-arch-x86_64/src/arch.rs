//! Implements the [`Arch`] architecture trait for the x86_64 architecture.

#![allow(clippy::inline_always)]

use crate::mem::{preboot_mapper::TranslatorMapper, recursive_mapper::RecursiveMapper};
use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
	ptr::from_ref,
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{
		PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator, PrebootAddressSpace,
		SupervisorAddressSegment, UnmapError,
	},
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
	type InterruptState = usize;
	type PrebootAddressSpace<P: PhysicalAddressTranslator> = TranslatorMapper<P>;
	type RuntimeAddressSpace = RecursiveMapper;

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

	unsafe fn prepare_transfer<P, A, C>(
		mapper: &Self::PrebootAddressSpace<P>,
		config: &PrebootConfig<C>,
		alloc: &mut A,
	) where
		P: PhysicalAddressTranslator,
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig,
	{
		// offset starts at L4/L4 index 255.
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

		// map the stubs into lower memory.
		let num_pages = (stub_len + 4095) >> 12;

		if let PrebootConfig::Primary {
			physical_address_translator,
			..
		} = config
		{
			let source = stub_start as *const u8;
			let dest = stubs_base as *mut u8;

			let current_mapper =
				Self::PrebootAddressSpace::get_current(physical_address_translator.clone());

			for page_offset in 0..num_pages {
				let phys = alloc
					.allocate()
					.expect("failed to allocate page for transfer stubs (out of memory)");

				let virt = stubs_base + page_offset * 4096;

				// Map into the target kernel page tables
				mapper
					.stubs()
					.map(alloc, virt, phys)
					.expect("failed to map page for transfer stubs for kernel address space");

				// Attempt to unmap it from the current address space.
				// If it's not mapped, we can ignore the error.
				current_mapper
					.stubs()
					.unmap(alloc, virt)
					.or_else(|e| {
						if e == UnmapError::NotMapped {
							Ok(0)
						} else {
							Err(e)
						}
					})
					.expect("failed to unmap page for transfer stubs from current address space");

				// Now map it into the current mapper so we can access it.
				current_mapper
					.stubs()
					.map(alloc, virt, phys)
					.expect("failed to map page for transfer stubs in current address space");
			}

			dest.copy_from(source, stub_len);

			Self::strong_memory_barrier();
		} else {
			// Simply invalidate the pages
			for page_offset in 0..num_pages {
				let virt = stubs_base + page_offset * 4096;
				crate::asm::invlpg(virt);
			}
		}
	}

	unsafe fn transfer<P>(
		entry: usize,
		mapper_token: <Self::PrebootAddressSpace<P> as PrebootAddressSpace<P>>::TransferToken,
	) -> !
	where
		P: PhysicalAddressTranslator,
	{
		let page_table_phys: u64 = mapper_token;
		let stack_addr: u64 = 0; // TODO(qix-): allocate stacks :D
		let stubs_addr: usize = crate::xfer::target_address();

		// Jump to stubs.
		asm!(
			"push {CR3_ADDR}",
			"push {STACK_ADDR}",
			"push {KERNEL_ENTRY}",
			"jmp {STUBS_ADDR}",
			CR3_ADDR = in(reg) page_table_phys,
			STACK_ADDR = in(reg) stack_addr,
			KERNEL_ENTRY = in(reg) entry,
			STUBS_ADDR = in(reg) stubs_addr,
			options(noreturn)
		);
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
	}
}
