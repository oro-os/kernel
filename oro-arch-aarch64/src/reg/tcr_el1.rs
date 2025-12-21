//! Provides definitions for the Translation Control Register (`TCR_EL1`).

use core::arch::asm;

use oro_kernel_macro::bitstruct;

bitstruct! {
	/// The `TCR_EL1` register.
	pub struct TcrEl1(u64) {
		/// This field affects whether a 52-bit output address can be described by the translation
		/// tables of the 4KB or 16KB translation granules.
		pub ds[59] => as bool,

		/// Top Byte ignored - indicates whether the top byte of an address is used for address
		/// match for the TTBR1_EL1 region, or ignored and used for tagged addresses.
		pub tbi1[38] => as bool,

		/// Top Byte ignored - indicates whether the top byte of an address is used for address
		/// match for the TTBR0_EL1 region, or ignored and used for tagged addresses.
		pub tbi0[37] => as bool,

		/// ASID size - the number of bits in the ASID field.
		pub as_size[36] => enum AsidSize(u64) {
			/// 8 bit - the upper 8 bits of `TTBR0_EL1` and `TTBR1_EL1` are ignored by hardware for
			/// every purpose except reading back the register, and are treated as if they are all zeros for
			/// when used for allocation and matching entries in the TLB.
			Bit8 = 0,
			/// 16 bit - the upper 16 bits of `TTBR0_EL1` and `TTBR1_EL1` are used for allocation and
			/// matching in the TLB.
			Bit16 = 1,
		},

		/// Intermediate Physical Address Size - the number of bits in the intermediate physical
		/// address.
		pub ips_bits[34:32] => enum PhysicalAddressSize(u64) {
			/// 32 bits - the physical address size is 32 bits.
			Bits32Gb4 = 0b000,
			/// 36 bits - the physical address size is 36 bits.
			Bits36Gb64 = 0b001,
			/// 40 bits - the physical address size is 40 bits.
			Bits40Tb1 = 0b010,
			/// 42 bits - the physical address size is 42 bits.
			Bits42Tb4 = 0b011,
			/// 44 bits - the physical address size is 44 bits.
			Bits44Tb16 = 0b100,
			/// 48 bits - the physical address size is 48 bits.
			Bits48Tb256 = 0b101,
			/// 52 bits - the physical address size is 52 bits.
			Bits52Tb4096 = 0b110,
			/// Reserved value
			Reserved = 0b111,
		},

		/// Granule size for TTBR1_EL1.
		pub tg1[31:30] => enum Tg1GranuleSize(u64) {
			/// Reserved value
			Reserved = 0b00,
			/// 16KiB granule size
			Kb16 = 0b01,
			/// 4KiB granule size
			Kb4 = 0b10,
			/// 64KiB granule size
			Kb64 = 0b11,
		},

		/// Shareability attribute for TTBR1_EL1.
		pub sh1[29:28] => enum Sh1Shareability(u64) {
			/// Non-shareable
			NonShareable = 0b00,
			/// Reserved value
			Reserved = 0b01,
			/// Outer shareable
			OuterShareable = 0b10,
			/// Inner shareable
			InnerShareable = 0b11,
		},

		/// Outer cacheability attribute for memory associated with translation table walks using
		/// TTBR1_EL1.
		pub orgn1[27:26] => enum Orgn1Cacheability(u64) {
			/// Normal memory, Non-cacheable
			NonCacheable = 0b00,
			/// Normal memory, Write-Back Write-Allocate Cacheable
			WriteBackWriteAllocate = 0b01,
			/// Normal memory, Write-Through Cacheable
			WriteThroughNoWriteAllocate = 0b10,
			/// Normal memory, Write-Back no Write-Allocate Cacheable
			WriteBackNoWriteAllocate = 0b11,
		},

		/// Inner cacheability attribute for memory associated with translation table walks using
		/// TTBR1_EL1.
		pub irgn1[25:24] => enum Irgn1Cacheability(u64) {
			/// Normal memory, Non-cacheable
			NonCacheable = 0b00,
			/// Normal memory, Write-Back Write-Allocate Cacheable
			WriteBackWriteAllocate = 0b01,
			/// Normal memory, Write-Through Cacheable
			WriteThroughNoWriteAllocate = 0b10,
			/// Normal memory, Write-Back no Write-Allocate Cacheable
			WriteBackNoWriteAllocate = 0b11,
		},

		/// Translation table walk disable for translations using TTBR1_EL1. When `true`, A TLB miss
		/// on an address that is translated using TTBR1_EL1 generates a Translation fault. No
		/// translation table walk is performed.
		pub epd1[23] => as bool,

		/// Selects whether `TTBR0_EL1` or `TTBR1_EL1` defines the ASID.
		pub a1[22] => enum AsidSelect(u64) {
			/// `TTBR0_EL1` defines the ASID.
			Ttbr0 = 0,
			/// `TTBR1_EL1` defines the ASID.
			Ttbr1 = 1,
		},

		/// Size offset of the memory region addressed by TTBR1_EL1. The region size is
		/// `2^(64-T1SZ)` bytes.
		pub t1sz[21:16] => as u8,

		/// Granule size for TTBR0_EL1.
		pub tg0[15:14] => enum Tg0GranuleSize(u64) {
			/// 4KiB granule size
			Kb4 = 0b00,
			/// 64KiB granule size
			Kb64 = 0b01,
			/// 16KiB granule size
			Kb16 = 0b10,
			/// Reserved value
			Reserved = 0b11,
		},

		/// Shareability attribute for TTBR0_EL1.
		pub sh0[13:12] => enum Sh0Shareability(u64) {
			/// Non-shareable
			NonShareable = 0b00,
			/// Reserved value
			Reserved = 0b01,
			/// Outer shareable
			OuterShareable = 0b10,
			/// Inner shareable
			InnerShareable = 0b11,
		},

		/// Outer cacheability attribute for memory associated with translation table walks using
		/// `TTBR0_EL1`
		pub orgn0[11:10] => enum Orgn0Cacheability(u64) {
			/// Normal memory, Non-cacheable
			NonCacheable = 0b00,
			/// Normal memory, Write-Back Write-Allocate Cacheable
			WriteBackWriteAllocate = 0b01,
			/// Normal memory, Write-Through Cacheable
			WriteThroughNoWriteAllocate = 0b10,
			/// Normal memory, Write-Back no Write-Allocate Cacheable
			WriteBackNoWriteAllocate = 0b11,
		},

		/// Inner cacheability attribute for memory associated with translation table walks using
		/// `TTBR0_EL1`
		pub irgn0[9:8] => enum Irgn0Cacheability(u64) {
			/// Normal memory, Non-cacheable
			NonCacheable = 0b00,
			/// Normal memory, Write-Back Write-Allocate Cacheable
			WriteBackWriteAllocate = 0b01,
			/// Normal memory, Write-Through Cacheable
			WriteThroughNoWriteAllocate = 0b10,
			/// Normal memory, Write-Back no Write-Allocate Cacheable
			WriteBackNoWriteAllocate = 0b11,
		},

		/// Translation table walk disable for translations using `TTBR0_EL1`. When `true`, A TLB
		/// miss on an address that is translated using `TTBR0_EL1` generates a Translation fault.
		/// No translation table walk is performed.
		pub epd0[7] => as bool,

		/// Size offset of the memory region addressed by `TTBR0_EL1`. The region size is
		/// `2^(64-T0SZ)` bytes.
		pub t0sz[5:0] => as u8,
	}
}

impl TcrEl1 {
	/// Loads the current `TCR_EL1` register value and returns a new instance.
	#[must_use]
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

	/// Returns the range of the TT0 address range based on `T0SZ`.
	///
	/// The upper bound is inclusive.
	#[must_use]
	pub fn tt0_range(self) -> (usize, usize) {
		(0, (1 << (64 - self.t0sz())) - 1)
	}

	/// Returns the range of the TT1 address range based on `T1SZ`.
	///
	/// The upper bound is inclusive.
	#[must_use]
	pub fn tt1_range(self) -> (usize, usize) {
		(
			0xFFFF_FFFF_FFFF_FFFF - (1 << (64 - self.t1sz())) + 1,
			0xFFFF_FFFF_FFFF_FFFF,
		)
	}
}
