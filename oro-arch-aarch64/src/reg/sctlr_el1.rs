//! Provides a high level abstraction over the system control register (`SCTLR_EL1`)
//! for AArch64.

use crate::reg::field::field32;
use core::arch::asm;

/// The `SCTLR_EL1` register.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct SctlrEl1(u32);

impl SctlrEl1 {
	field32!(
		uci,
		26,
		"When set, enables EL0 access in AArch64 for DC CVAU, DC CIVAC, DC CVAC, and IC IVAU \
		 instructions."
	);

	field32!(
		ee,
		25,
		Endianness,
		"Exception Endianness. This bit controls the endianness for explicit data accesses at \
		 EL1, and stage 1 translation table walks at EL1 and EL0."
	);

	field32!(
		e0e,
		24,
		Endianness,
		"Endianness of explicit data accesses at EL0"
	);

	field32!(
		wxn,
		19,
		"When `true`, write permissions imply execute-never."
	);

	field32!(
		ntwe,
		18,
		"When `true`, EL0 is allowed to execute `WFE`. When `false`, if a WFE instruction \
		 executed at EL0 would cause execution to be suspended, such as if the event register is \
		 not set and there is not a pending WFE wakeup event, it is taken as an exception to EL1 \
		 using the 0x1 ESR code."
	);

	field32!(
		ntwi,
		16,
		"When `true`, EL0 is allowed to execute `WFI`. When `false`, if a WFI instruction \
		 executed at EL0 would cause execution to be suspended, such as if there is not a pending \
		 WFI wakeup event, it is taken as an exception to EL1 using the 0x1 ESR code."
	);

	field32!(
		uct,
		15,
		"When `true`, allows EL0 to access the `CTR_EL0` register."
	);

	field32!(
		dze,
		14,
		"When `true`, EL0 is allowed to execute the `DC ZVA` instruction."
	);

	field32!(
		i,
		12,
		"Instruction cache enable. When `true`, both EL0 and EL1 instruction caches are set to \
		 Inner/Outer Write-Through. Otherwise, they are Inner/Outer Non-Cacheable."
	);

	field32!(
		uma,
		9,
		"When `true`, EL0 is allowed to access the interrupt masks."
	);

	field32!(
		sed,
		8,
		"`SETEND` disable. When `true`, the `SETEND` (set endianness of data retrievals) \
		 instruction is undefined."
	);

	field32!(
		itd,
		7,
		"IT disable. When `true`, IT becomes implementation-defined. See D8.2 of the ARMv8 manual \
		 for more information."
	);

	field32!(thee, 6, "T32EE enable. When `true`, T32EE is enabled.");

	field32!(
		cp15ben,
		5,
		"CP15 barrier enable. When `true`, AArch32 CP15 barriers are enabled."
	);

	field32!(
		sa0,
		4,
		"Stack Alignment Check Enable for EL0. When `true`, use of the stack pointer as the base \
		 address in a load/store instruction at this register's exception level must be aligned \
		 to a 16-byte boundary, or a Stack Alignment Fault exception will be raised."
	);

	field32!(
		sa,
		3,
		"Stack Alignment Check Enable. When `true`, use of the stack pointer as the base address \
		 in a load/store instruction at any exception level must be aligned to a 16-byte \
		 boundary, or a Stack Alignment Fault exception will be raised."
	);

	field32!(
		c,
		2,
		"Cache enable. This is an enable bit for data and unified caches at EL0 and EL1."
	);

	field32!(
		a,
		1,
		"Alignment check enable. This is the enable bit for Alignment fault checking. When \
		 `true`, all instructions that load or store one or more registers have an alignment \
		 check that the address being accessed is aligned to the size of the data element(s) \
		 being accessed. If this check fails it causes an Alignment fault, which is taken as a \
		 Data Abort exception."
	);

	field32!(
		m,
		0,
		"MMU enable. This is the enable bit for the MMU. When `true`, the MMU is enabled. When \
		 `false`, the MMU is disabled and the core performs address translation using the \
		 identity map."
	);

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

	/// Creates a new `SCTLR_EL1` register with all fields set to `0`.
	#[expect(clippy::new_without_default)]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}
}

impl From<u32> for SctlrEl1 {
	fn from(val: u32) -> Self {
		Self(val)
	}
}

impl From<SctlrEl1> for u32 {
	fn from(val: SctlrEl1) -> Self {
		val.0
	}
}

impl core::fmt::Debug for SctlrEl1 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("SctlrEl1")
			.field("uci", &self.uci())
			.field("ee", &self.ee())
			.field("e0e", &self.e0e())
			.field("wxn", &self.wxn())
			.field("ntwe", &self.ntwe())
			.field("ntwi", &self.ntwi())
			.field("uct", &self.uct())
			.field("dze", &self.dze())
			.field("i", &self.i())
			.field("uma", &self.uma())
			.field("sed", &self.sed())
			.field("itd", &self.itd())
			.field("thee", &self.thee())
			.field("cp15ben", &self.cp15ben())
			.field("sa0", &self.sa0())
			.field("sa", &self.sa())
			.field("c", &self.c())
			.field("a", &self.a())
			.field("m", &self.m())
			.finish()
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
