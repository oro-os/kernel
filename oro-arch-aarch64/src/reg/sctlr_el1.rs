//! Provides a high level abstraction over the system control register (`SCTLR_EL1`)
//! for AArch64.

use core::arch::asm;

use oro_kernel_macro::bitstruct;

bitstruct! {
	/// The `SCTLR_EL1` register.
	pub struct SctlrEl1(u32) {
		/// When set, enables EL0 access in AArch64 for DC CVAU, DC CIVAC, DC CVAC, and IC IVAU instructions.
		pub uci[26] => as bool,

		/// Exception Endianness. This bit controls the endianness for explicit data accesses at EL1, and stage 1 translation table walks at EL1 and EL0.
		pub ee[25] => From<Endianness>,

		/// Endianness of explicit data accesses at EL0
		pub e0e[24] => From<Endianness>,

		/// When `true`, write permissions imply execute-never.
		pub wxn[19] => as bool,

		/// When `true`, EL0 is allowed to execute `WFE`. When `false`, if a WFE instruction executed at EL0 would cause execution to be suspended, such as if the event register is not set and there is not a pending WFE wakeup event, it is taken as an exception to EL1 using the 0x1 ESR code.
		pub ntwe[18] => as bool,

		/// When `true`, EL0 is allowed to execute `WFI`. When `false`, if a WFI instruction executed at EL0 would cause execution to be suspended, such as if there is not a pending WFI wakeup event, it is taken as an exception to EL1 using the 0x1 ESR code.
		pub ntwi[16] => as bool,

		/// When `true`, allows EL0 to access the `CTR_EL0` register.
		pub uct[15] => as bool,

		/// When `true`, EL0 is allowed to execute the `DC ZVA` instruction.
		pub dze[14] => as bool,

		/// Instruction cache enable. When `true`, both EL0 and EL1 instruction caches are set to Inner/Outer Write-Through. Otherwise, they are Inner/Outer Non-Cacheable.
		pub i[12] => as bool,

		/// When `true`, EL0 is allowed to access the interrupt masks.
		pub uma[9] => as bool,

		/// `SETEND` disable. When `true`, the `SETEND` (set endianness of data retrievals) instruction is undefined.
		pub sed[8] => as bool,

		/// IT disable. When `true`, IT becomes implementation-defined. See D8.2 of the ARMv8 manual for more information.
		pub itd[7] => as bool,

		/// T32EE enable. When `true`, T32EE is enabled.
		pub thee[6] => as bool,

		/// CP15 barrier enable. When `true`, AArch32 CP15 barriers are enabled.
		pub cp15ben[5] => as bool,

		/// Stack Alignment Check Enable for EL0. When `true`, use of the stack pointer as the base address in a load/store instruction at this register's exception level must be aligned to a 16-byte boundary, or a Stack Alignment Fault exception will be raised.
		pub sa0[4] => as bool,

		/// Stack Alignment Check Enable. When `true`, use of the stack pointer as the base address in a load/store instruction at any exception level must be aligned to a 16-byte boundary, or a Stack Alignment Fault exception will be raised.
		pub sa[3] => as bool,

		/// Cache enable. This is an enable bit for data and unified caches at EL0 and EL1.
		pub c[2] => as bool,

		/// Alignment check enable. This is the enable bit for Alignment fault checking. When `true`, all instructions that load or store one or more registers have an alignment check that the address being accessed is aligned to the size of the data element(s) being accessed. If this check fails it causes an Alignment fault, which is taken as a Data Abort exception.
		pub a[1] => as bool,

		/// MMU enable. This is the enable bit for the MMU. When `true`, the MMU is enabled. When `false`, the MMU is disabled and the core performs address translation using the identity map.
		pub m[0] => as bool,
	}
}

impl SctlrEl1 {
	/// Loads the current `SCTLR_EL1` register value and returns a new instance.
	#[must_use]
	pub fn load() -> Self {
		unsafe {
			let mut sctlr_el1: u64;
			asm!(
				"mrs {0:x}, SCTLR_EL1",
				out(reg) sctlr_el1
			);
			// SAFETY(qix-): SCTLR_EL1 is a 32-bit register. Truncating the bits makes no difference.
			#[expect(clippy::cast_possible_truncation)]
			Self(sctlr_el1 as u32)
		}
	}

	/// Writes the `SCTLR_EL1` register with the value in this struct.
	pub fn write(self) {
		unsafe {
			asm!(
				"msr SCTLR_EL1, {0:x}",
				in(reg) u64::from(self.0)
			);
		}
	}
}

/// Specifies the endianness for explicit data accesses, for both the `ee` and `e0e` fields of the `SCTLR_EL1` register.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Endianness {
	/// Little-endian.
	Little = 0,
	/// Big-endian.
	Big    = 1,
}

impl From<Endianness> for u32 {
	fn from(val: Endianness) -> Self {
		val as u32
	}
}

impl From<bool> for Endianness {
	fn from(val: bool) -> Self {
		if val {
			Endianness::Big
		} else {
			Endianness::Little
		}
	}
}

impl From<u32> for Endianness {
	fn from(val: u32) -> Self {
		match val & 1 {
			0 => Endianness::Little,
			_ => Endianness::Big,
		}
	}
}
