//! # Oro kernel CRC64 calculator
//! This crate provides a CRC64 implementation for use in
//! the Oro kernel. It uses the ECMA-182 polynomial (`0x42F0E1EBA9EA3693`)
//! and an initial value of `0` (catalog `CRC64/ECMA-182`).
//!
//! Note that the ECMA-182 polynomial is not reflected when generating
//! the lookup table (neither the inputs nor the outputs are reflected).
#![cfg_attr(not(test), no_std)]

/// Generates the CRC64 lookup table for the given polynomial.
const fn generate_lut(polynomial: u64) -> [u64; 256] {
	let mut table: [u64; 256] = [0; 256];
	let mut i: usize = 0;
	while i < 256 {
		let mut v = i as u64;
		let mut j: usize = 0;
		while j < 64 {
			v = if (v & 0x8000000000000000) != 0 {
				(v << 1) ^ polynomial
			} else {
				v << 1
			};
			j += 1;
		}
		table[i] = v;
		i += 1;
	}
	table
}

/// The ECMA-182 polynomial for CRC64.
const CRC64_ECMA182_POLY: u64 = 0x42F0E1EBA9EA3693;

/// The CRC64 lookup table for the ECMA-182 polynomial.
const CRC64_ECMA182_TABLE: [u64; 256] = generate_lut(CRC64_ECMA182_POLY);

/// Running CRC64 calculator.
pub struct Crc64 {
	value: u64,
}

impl Default for Crc64 {
	fn default() -> Self {
		Crc64::new()
	}
}

impl Crc64 {
	/// Creates a new CRC64 calculator with the initial value.
	pub fn new() -> Crc64 {
		Self { value: 0 }
	}

	/// Resets the CRC64 calculator to the initial value.
	pub fn reset(&mut self) {
		self.value = 0;
	}

	/// Updates the CRC64 calculator with the given buffer.
	pub fn update(&mut self, buf: &[u8]) {
		for &i in buf {
			self.value =
				CRC64_ECMA182_TABLE[(((self.value >> 56) as u8) ^ i) as usize] ^ (self.value << 8);
		}
	}

	/// Finalizes the CRC64 calculation and returns the checksum.
	pub fn finalize(&mut self) -> u64 {
		self.value
	}

	/// Calculates the CRC64 checksum of the given buffer as a one-shot operation.
	pub fn checksum(buf: &[u8]) -> u64 {
		let mut crc = Crc64::new();
		crc.update(buf);
		crc.finalize()
	}
}

#[expect(non_snake_case)]
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sparse_lookup_check() {
		assert_eq!(CRC64_ECMA182_TABLE[0x00], 0x0000000000000000);
		assert_eq!(CRC64_ECMA182_TABLE[0x01], 0x42F0E1EBA9EA3693);
		assert_eq!(CRC64_ECMA182_TABLE[0x44], 0x93366450E42ECDF0);
		assert_eq!(CRC64_ECMA182_TABLE[0x5D], 0x259D31CEC1A0A732);
		assert_eq!(CRC64_ECMA182_TABLE[0x77], 0xFE60CF6CAF321874);
		assert_eq!(CRC64_ECMA182_TABLE[0x98], 0x02A151B5F156289C);
		assert_eq!(CRC64_ECMA182_TABLE[0xA2], 0xBF61D7E80F251235);
		assert_eq!(CRC64_ECMA182_TABLE[0xBC], 0x87E8C60FDED7CF9D);
		assert_eq!(CRC64_ECMA182_TABLE[0xD5], 0x41011884A0170A41);
		assert_eq!(CRC64_ECMA182_TABLE[0xFE], 0xD80C07CD676F8394);
		assert_eq!(CRC64_ECMA182_TABLE[0xFF], 0x9AFCE626CE85B507);
	}

	#[test]
	fn check() {
		static DATA: &[u8] = b"123456789";
		let crc = Crc64::checksum(DATA);
		assert_eq!(crc, 0x6C40DF5F0B497347);
	}

	#[test]
	fn oro_operating_system() {
		static DATA: &[u8] = b"Oro Operating System";
		let crc = Crc64::checksum(DATA);
		assert_eq!(crc, 0x90C960325295BA7E);
	}

	#[test]
	fn data256() {
		static DATA: &[u8] = include_bytes!("../test-data/data256");
		let crc = Crc64::checksum(DATA);
		assert_eq!(crc, 0x18DB68B6B41ACDAA);
	}

	#[test]
	fn data4MiB() {
		static DATA: &[u8] = include_bytes!("../test-data/data4MiB");
		let crc = Crc64::checksum(DATA);
		assert_eq!(crc, 0xBA23A406BE77E0B4);
	}
}
