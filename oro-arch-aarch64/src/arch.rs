//! Main [`Arch`] implementation for the Aarch64 architecture.

#![allow(clippy::inline_always)]

use crate::mem::mapper::{kernel::KernelMapper, preboot::PrebootMapper};
use core::{
	arch::asm,
	fmt::{self, Write},
	mem::MaybeUninit,
};
use oro_common::{
	elf::{ElfClass, ElfEndianness, ElfMachine},
	mem::{PageFrameAllocate, PageFrameFree, PhysicalAddressTranslator},
	sync::UnfairCriticalSpinlock,
	Arch, PrebootConfig, PrebootPrimaryConfig,
};
use oro_serial_pl011 as pl011;

/// The shared serial port for the system.
///
/// **NOTE:** This is a temporary solution until pre-boot module loading
static mut SERIAL: UnfairCriticalSpinlock<Aarch64, MaybeUninit<pl011::PL011>> =
	UnfairCriticalSpinlock::new(MaybeUninit::uninit());

/// aarch64 architecture support implementation for the Oro kernel.
pub struct Aarch64;

unsafe impl Arch for Aarch64 {
	type InterruptState = usize;
	type PrebootAddressSpace<P: PhysicalAddressTranslator> = PrebootMapper<P>;
	type RuntimeAddressSpace = KernelMapper;
	type TransferToken = ();

	const ELF_CLASS: ElfClass = ElfClass::Class64;
	const ELF_ENDIANNESS: ElfEndianness = ElfEndianness::Little;
	const ELF_MACHINE: ElfMachine = ElfMachine::Aarch64;

	unsafe fn init_shared() {
		// TODO(qix-): This is set up specifically for QEMU.
		// TODO(qix-): This will need to be adapted to handle
		// TODO(qix-): different UART types and a configurable
		// TODO(qix-): base address / settings in the future.
		SERIAL.lock().write(pl011::PL011::new::<Self>(
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
			let mut lock = SERIAL.lock();
			writeln!(lock.assume_init_mut(), "{message}")
		}
		.unwrap();
	}

	unsafe fn prepare_transfer<P, A, C>(
		_mapper: Self::PrebootAddressSpace<P>,
		_config: &PrebootConfig<C>,
		_alloc: &mut A,
	) -> Self::TransferToken
	where
		P: PhysicalAddressTranslator,
		A: PageFrameAllocate + PageFrameFree,
		C: PrebootPrimaryConfig,
	{
		todo!();
	}

	unsafe fn transfer(_entry: usize, _transfer_token: Self::TransferToken) -> ! {
		todo!();
	}
}
