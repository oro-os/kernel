//! Provides definitions for the AArch64 Memory Model Feature Register 0 (EL1) (`ID_AA64MMFR0_EL1`).

use core::arch::asm;

use orok_macro::bitstruct;

bitstruct! {
	/// The `ID_AA64MMFR0_EL1` register.
	pub struct IdAa64Mmfr0El1(u64) {
		/// Support for 4kbyte translation granules.
		#[non_exhaustive]
		pub tgran4[31:28] => enum Tgran4Support(u8) {
			/// 4kbyte translation granules are supported.
			Supported = 0b0000,
			/// 4kbyte translation granules are not supported.
			NotSupported = 0b1111,
		},

		/// Support for 64kbyte translation granules.
		#[non_exhaustive]
		pub tgran64[27:24] => enum Tgran64Support(u8) {
			/// 64kbyte translation granules are supported.
			Supported = 0b0000,
			/// 64kbyte translation granules are not supported.
			NotSupported = 0b1111,
		},

		/// Support for 16kbyte translation granules.
		#[non_exhaustive]
		pub tgran16[23:20] => enum Tgran16Support(u8) {
			/// 16kbyte translation granules are not supported.
			NotSupported = 0b0000,
			/// 16kbyte translation granules are supported.
			Supported = 0b0001,
		},

		/// Mixed-endian support at EL0 only.
		///
		/// Should only be considered if [`IdAa64Mmfr0El1::bigend`] is `Supported`.
		#[non_exhaustive]
		pub bigendel0[19:16] => enum BigEndEl0Support(u8) {
			/// No support for mixed-endian at EL0.
			///
			/// The `SCTLR_EL1.E0E` bit has a fixed value.
			NoSupport = 0b0000,
			/// Support for mixed-endian at EL0.
			///
			/// The `SCTLR_EL1.E0E` bit controls the endianness of data accesses at EL0.
			Supported = 0b0001,
		},

		/// Secure versus nonsecure memory disctinction.
		#[non_exhaustive]
		pub snsmem[15:12] => enum SnsmemDistinction(u8) {
			/// No distinction between secure and nonsecure memory.
			///
			/// All memory accesses are treated as nonsecure.
			NoDistinction = 0b0000,
			/// Distinction between secure and nonsecure memory.
			///
			/// Memory accesses are treated as secure or nonsecure based on the security state of the
			/// core and the security attribute of the memory region being accessed.
			Distinction = 0b0001,
		},

		/// Mixed-endian configuration support.
		#[non_exhaustive]
		pub bigend[11:8] => enum BigEndSupport(u8) {
			/// No support for mixed-endian configuration.
			///
			/// The `SCTLR_ELn.EE` bits have a fixed value.
			NoSupport = 0b0000,
			/// Support for mixed-endian configuration.
			///
			/// The `SCTLR_ELn.EE` bits controls the endianness of
			/// data accesses at all exception levels, except EL0,
			/// which is controlled by `SCTLR_EL1.E0E` if
			/// `ID_AA64MMFR0_EL1.BIGENDEL0` is `Supported`.
			Supported = 0b0001,
		},

		/// Number of ASID bits.
		#[non_exhaustive]
		pub asidbits[7:4] => enum AsidBitCount(u8) {
			/// 8 bits of ASID.
			Bits8 = 0b0000,
			/// 16 bits of ASID.
			Bits16 = 0b0010,
		},

		/// The physical address range supported by the system.
		#[non_exhaustive]
		pub parange[3:0] => enum PhysicalAddressRange(u8) {
			/// 32 bits of physical address.
			Bits32 = 0b0000,
			/// 36 bits of physical address.
			Bits36 = 0b0001,
			/// 40 bits of physical address.
			Bits40 = 0b0010,
			/// 42 bits of physical address.
			Bits42 = 0b0011,
			/// 44 bits of physical address.
			Bits44 = 0b0100,
			/// 48 bits of physical address.
			Bits48 = 0b0101,
			/// 52 bits of physical address.
			Bits52 = 0b0110,
		},
	}
}

#[cfg(test)]
mod fake {
	use orok_type::RelaxedU64;

	use super::*;

	static FAKE: RelaxedU64 = RelaxedU64::new(0);

	impl IdAa64Mmfr0El1 {
		/// Loads the `ID_AA64MMFR0_EL1` register.
		#[inline(always)]
		#[must_use = "this function has no side effects; it makes no sense to discard the return \
		              value"]
		pub fn load() -> Self {
			Self(FAKE.load())
		}

		/// Stores the `ID_AA64MMFR0_EL1` register.
		///
		/// **Testing only;** the real `ID_AA64MMFR0_EL1` register is read-only and cannot be written to.
		#[inline(always)]
		pub fn store(self) {
			FAKE.store(self.0);
		}
	}
}

#[cfg(not(test))]
impl IdAa64Mmfr0El1 {
	/// Loads the current `ID_AA64MMFR0_EL1` register value and returns a new instance.
	#[must_use = "this function has no side effects; it makes no sense to discard the return value"]
	pub fn load() -> Self {
		// SAFETY: This register is safe to load as it has no side effects.
		unsafe {
			let mut id_aa64mmfr0_el1: u64;
			asm!(
				"mrs {0:x}, ID_AA64MMFR0_EL1",
				out(reg) id_aa64mmfr0_el1,
				options(pure, nostack, preserves_flags, readonly),
			);
			Self(id_aa64mmfr0_el1)
		}
	}
}
