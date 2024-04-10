//! Implements the [`Arch`] architecture trait for the `x86_64` architecture.

#![allow(clippy::inline_always)]

use crate::mem::{preboot_mapper::TranslatorMapper, recursive_mapper::RecursiveMapper};
use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::PhysicalAddressTranslator,
	sync::UnfairCriticalSpinlock,
	Arch,
};
use uart_16550::SerialPort;

/// The shared serial port for the system.
///
/// **NOTE:** This is a temporary solution until pre-boot module loading
/// is implemented.
static SERIAL: UnfairCriticalSpinlock<X86_64, MaybeUninit<SerialPort>> =
	UnfairCriticalSpinlock::new(MaybeUninit::uninit());

/// `x86_64` architecture support implementation for the Oro kernel.
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

	unsafe fn prepare_transfer<P, A>(_mapper: &Self::PrebootAddressSpace<P>, _alloc: &mut A)
	where
		P: PhysicalAddressTranslator,
		A: oro_common::mem::PageFrameAllocate + oro_common::mem::PageFrameFree,
	{
		// TODO(qix-)
		panic!("x86_64::prepare_transfer() is not implemented");
	}

	unsafe fn transfer<P>(
		_entry: usize,
		_mapper_token: <Self::PrebootAddressSpace<P> as oro_common::mem::PrebootAddressSpace<P>>::TransferToken,
	) -> !
	where
		P: PhysicalAddressTranslator,
	{
		// TODO(qix-)
		panic!("x86_64::transfer() is not implemented");
	}

	#[inline(always)]
	fn strong_memory_barrier() {
		unsafe {
			core::arch::asm!("mfence", options(nostack, preserves_flags),);
		}
	}
}
