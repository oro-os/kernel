//! Provides definitions for the Translation Control Register (`TCR_EL1`).

use crate::reg::field::field;
use core::arch::asm;
use oro_common::proc::AsU64;

/// The `TCR_EL1` register.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct TcrEl1(u64);

impl TcrEl1 {
	field!(
		ds,
		59,
		"This field affects whether a 52-bit output address can be described by the translation \
		 tables of the 4KB or 16KB translation granules."
	);

	field!(
		tbi1,
		38,
		"Top Byte ignored - indicates whether the top byte of an address is used for address \
		 match for the TTBR1_EL1 region, or ignored and used for tagged addresses."
	);

	field!(
		tbi0,
		37,
		"Top Byte ignored - indicates whether the top byte of an address is used for address \
		 match for the TTBR0_EL1 region, or ignored and used for tagged addresses."
	);

	field!(
		as_size,
		36,
		AsidSize,
		"ASID size - the number of bits in the ASID field."
	);

	field!(
		ips_bits,
		32,
		34,
		PhysicalAddressSize,
		"Intermediate Physical Address Size - the number of bits in the intermediate physical \
		 address."
	);

	field!(tg1, 30, 31, Tg1GranuleSize, "Granule size for TTBR1_EL1.");

	field!(
		sh1,
		28,
		29,
		Shareability,
		"Shareability attribute for TTBR1_EL1."
	);

	field!(
		orgn1,
		26,
		27,
		Cacheability,
		"Outer cacheability attribute for memory associated with translation table walks using \
		 TTBR1_EL1."
	);

	field!(
		irgn1,
		24,
		25,
		Cacheability,
		"Inner cacheability attribute for memory associated with translation table walks using \
		 TTBR1_EL1."
	);

	field!(
		epd1,
		23,
		"Translation table walk disable for translations using TTBR1_EL1. When `true`, A TLB miss \
		 on an address that is translated using TTBR1_EL1 generates a Translation fault. No \
		 translation table walk is performed."
	);

	field!(
		a1,
		22,
		AsidSelect,
		"Selects whether `TTBR0_EL1` or `TTBR1_EL1` defines the ASID."
	);

	field!(
		t1sz,
		16,
		21,
		"Size offset of the memory region addressed by TTBR1_EL1. The region size is \
		 `2^(64-T1SZ)` bytes."
	);

	field!(tg0, 14, 15, Tg0GranuleSize, "Granule size for TTBR0_EL1.");

	field!(
		sh0,
		12,
		13,
		Shareability,
		"Shareability attribute for TTBR0_EL1."
	);

	field!(
		orgn0,
		10,
		11,
		Cacheability,
		"Outer cacheability attribute for memory associated with translation table walks using \
		 `TTBR0_EL1`"
	);

	field!(
		irgn0,
		8,
		9,
		Cacheability,
		"Inner cacheability attribute for memory associated with translation table walks using \
		 `TTBR0_EL1`"
	);

	field!(
		epd0,
		7,
		"Translation table walk disable for translations using `TTBR0_EL1`. When `true`, A TLB \
		 miss on an address that is translated using `TTBR0_EL1` generates a Translation fault. \
		 No translation table walk is performed."
	);

	field!(
		t0sz,
		0,
		5,
		"Size offset of the memory region addressed by `TTBR0_EL1`. The region size is \
		 `2^(64-T0SZ)` bytes."
	);

	/// Loads the current `TCR_EL1` register value and returns a new instance.
	pub fn load() -> Self {
		unsafe {
			let mut tcr_el1: u64;
			asm!(
				"mrs {0:x}, TCR_EL1",
				out(reg) tcr_el1
			);
			Self(tcr_el1)
		}
	}

	/// Writes the `TCR_EL1` register with the value in this struct.
	pub fn write(self) {
		unsafe {
			asm!(
				"msr TCR_EL1, {0:x}",
				in(reg) self.0
			);
		}
	}

	/// Creates a new `TCR_EL1` register with all fields set to `0`.
	#[allow(clippy::new_without_default)]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Returns the range of the TT0 address range based on `T0SZ`.
	///
	/// The upper bound is inclusive.
	pub fn tt0_range(self) -> (usize, usize) {
		(0, (1 << (64 - self.t0sz())) - 1)
	}

	/// Returns the range of the TT1 address range based on `T1SZ`.
	///
	/// The upper bound is inclusive.
	pub fn tt1_range(self) -> (usize, usize) {
		(
			0xFFFF_FFFF_FFFF_FFFF - (1 << (64 - self.t1sz())) + 1,
			0xFFFF_FFFF_FFFF_FFFF,
		)
	}
}

impl From<u64> for TcrEl1 {
	fn from(val: u64) -> Self {
		Self(val)
	}
}

impl From<TcrEl1> for u64 {
	fn from(val: TcrEl1) -> Self {
		val.0
	}
}

impl core::fmt::Debug for TcrEl1 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("TcrEl1")
			.field("ds", &self.ds())
			.field("tbi1", &self.tbi1())
			.field("tbi0", &self.tbi0())
			.field("as_size", &self.as_size())
			.field("ips_bits", &self.ips_bits())
			.field("tg1", &self.tg1())
			.field("sh1", &self.sh1())
			.field("orgn1", &self.orgn1())
			.field("irgn1", &self.irgn1())
			.field("epd1", &self.epd1())
			.field("a1", &self.a1())
			.field("t1sz", &self.t1sz())
			.field("tg0", &self.tg0())
			.field("sh0", &self.sh0())
			.field("orgn0", &self.orgn0())
			.field("irgn0", &self.irgn0())
			.field("epd0", &self.epd0())
			.field("t0sz", &self.t0sz())
			.finish()
	}
}

/// The size of the ASIDs. Used in the `as` (codified as
/// [`TcrEl1::as_size`]) field of the `TCR_EL1` register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsidSize {
	/// 8 bit - the upper 8 bits of `TTBR0_EL1` and `TTBR1_EL1` are ignored by hardware for
	/// every purpose except reading back the register, and are treated as if they are all zeros for
	/// when used for allocation and matching entries in the TLB.
	Bit8,
	/// 16 bit - the upper 16 bits of `TTBR0_EL1` and `TTBR1_EL1` are used for allocation and
	/// matching in the TLB.
	Bit16,
}

impl From<AsidSize> for u64 {
	fn from(val: AsidSize) -> Self {
		match val {
			AsidSize::Bit8 => 0,
			AsidSize::Bit16 => 1,
		}
	}
}

impl From<bool> for AsidSize {
	fn from(val: bool) -> Self {
		if val { AsidSize::Bit16 } else { AsidSize::Bit8 }
	}
}

/// The physical address size. Used in the `ips` (codified as
/// [`TcrEl1::ips_bits`]) field of the `TCR_EL1` register.
#[derive(Debug, Clone, Copy, AsU64, PartialEq, Eq)]
#[repr(u64)]
pub enum PhysicalAddressSize {
	/// 32 bits - the physical address size is 32 bits.
	Bits32Gb4   = 0b000,
	/// 36 bits - the physical address size is 36 bits.
	Bits36Gb64  = 0b001,
	/// 40 bits - the physical address size is 40 bits.
	Bits40Tb1   = 0b010,
	/// 42 bits - the physical address size is 42 bits.
	Bits42Tb4   = 0b011,
	/// 44 bits - the physical address size is 44 bits.
	Bits44Tb16  = 0b100,
	/// 48 bits - the physical address size is 48 bits.
	Bits48Tb256 = 0b101,
}

/// Granule size value for the `TG1` field of the `TCR_EL1` register.
#[derive(Debug, Clone, Copy, AsU64, PartialEq, Eq)]
#[repr(u64)]
pub enum Tg1GranuleSize {
	/// 16KiB granule size
	Kb16 = 0b01,
	/// 4KiB granule size
	Kb4  = 0b10,
	/// 64KiB granule size
	Kb64 = 0b11,
}

/// Granule size value for the `TG0` field of the `TCR_EL1` register.
#[derive(Debug, Clone, Copy, AsU64, PartialEq, Eq)]
#[repr(u64)]
pub enum Tg0GranuleSize {
	/// 4KiB granule size
	Kb4  = 0b00,
	/// 64KiB granule size
	Kb64 = 0b01,
	/// 16KiB granule size
	Kb16 = 0b10,
}

/// Shareability attributes for the SH1 and SH0 fields of the `TCR_EL1` register.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, AsU64, PartialEq, Eq)]
#[repr(u64)]
pub enum Shareability {
	/// Non-shareable
	NonShareable   = 0b00,
	/// Outer shareable
	OuterShareable = 0b10,
	/// Inner shareable
	InnerShareable = 0b11,
}

/// Inner and Outer cacheability attributes for the ORGN1 / IRGN1 fields of the `TCR_EL1` register.
#[derive(Debug, Clone, Copy, AsU64, PartialEq, Eq)]
#[repr(u64)]
pub enum Cacheability {
	/// Normal memory, Non-cacheable
	NonCacheable                = 0b00,
	/// Normal memory, Write-Back Write-Allocate Cacheable
	WriteBackWriteAllocate      = 0b01,
	/// Normal memory, Write-Through Cacheable
	WriteThroughNoWriteAllocate = 0b10,
	/// Normal memory, Write-Back no Write-Allocate Cacheable
	WriteBackNoWriteAllocate    = 0b11,
}

/// Selects which register - either `TTBR0_EL1` or `TTBR1_EL1` - defines the ASID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum AsidSelect {
	/// `TTBR0_EL1` defines the ASID.
	Ttbr0 = 0,
	/// `TTBR1_EL1` defines the ASID.
	Ttbr1 = 1,
}

impl From<AsidSelect> for u64 {
	fn from(val: AsidSelect) -> Self {
		val as u64
	}
}

impl From<bool> for AsidSelect {
	fn from(val: bool) -> Self {
		if val {
			AsidSelect::Ttbr1
		} else {
			AsidSelect::Ttbr0
		}
	}
}
