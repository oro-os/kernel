//! # Oro Operating System kernel ramdisk format
//! This crate provides the encoder (for `std`-enabled builds)
//! and decoder (for `no_std` builds) for the Oro kernel ramdisk format.
//!
//! Oro's ramdisk format is a simple, self-contained archive format that
//! is used to package not only the kernel itself, but also any root modules
//! that the kernel may need to load at runtime.
//!
//! The format is a fourcc-based format with a single header, index block,
//! and data blocks.
//!
//! All blocks must be aligned to 8-byte boundaries.
//!
//! All CRC64 checksums are calculated using the ECMA-182 polynomial (0x42F0E1EBA9EA3693)
//! and the initial value of 0xFFFFFFFFFFFFFFFF. The checksum is calculated over the entire block,
//! including the reserved region, but excluding the checksum field itself (substituting zeroes).
//! See the `oro-crc64` crate for more information on how Oro calculates CRC64 checksums.
//!
//! ## Header
//! The header is the first bit of data in the ramdisk file,
//! and is used to identify the file as a ramdisk, as well as provide
//! some basic information about the file, such as its total length and
//! the offset to the root object, the index.
//!
//! See the `header` module sources for more information on the header and its fields.
//!
//! ## Index
//! The index block is the 'root' of the ramdisk's object tree. The object tree
//! is a flat list of objects that are stored in the ramdisk, and is used to locate objects
//! within the file.

//! ## Items
//! For a list of all other items and their fields, see the [`item`] module.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

pub mod item;

#[cfg(feature = "encoder")]
pub(crate) mod chunked_stream;
#[cfg(feature = "decoder")]
mod decoder;
#[cfg(feature = "encoder")]
mod encoder;
mod endian;
mod error;
mod header;

pub(crate) use endian::*;

#[cfg(feature = "decoder")]
pub use self::decoder::*;
#[cfg(feature = "encoder")]
pub use self::{encoder::*, error::EncoderError};
pub use self::{
	error::{Error, ItemError},
	header::*,
};

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(all(feature = "encoder", feature = "decoder"))]
	#[test]
	fn roundtrip_empty() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let written = Encoder::new(&mut cursor).unwrap().finish().unwrap();
		assert_eq!(
			written as usize,
			core::mem::size_of::<crate::header::Header>()
				+ core::mem::size_of::<crate::item::Index>()
		);

		let decoder = Decoder::new(&out).unwrap();
		assert!(decoder.index_items().is_empty());
	}

	#[cfg(all(
		feature = "encoder",
		feature = "decoder",
		feature = "compression",
		feature = "decompression"
	))]
	#[test]
	fn roundtrip_empty_kernel_uncompressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let kernel: Vec<u8> = vec![];
		let mut kernel_cursor = std::io::Cursor::new(kernel);
		encoder
			.write_item(crate::item::KernelEncoder::new_uncompressed(
				&mut kernel_cursor,
				crate::item::KernelArch::Riscv64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);

		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, false);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Riscv64);
		assert!(kernel.data.is_empty());
	}

	#[cfg(all(feature = "encoder", feature = "decoder"))]
	#[test]
	fn roundtrip_empty_kernel_compressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let kernel: Vec<u8> = vec![];
		let mut kernel_cursor = std::io::Cursor::new(kernel);
		encoder
			.write_item(crate::item::KernelEncoder::new_compress_with_zstd(
				&mut kernel_cursor,
				3,
				crate::item::KernelArch::Aarch64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);
		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, true);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Aarch64);

		let mut stream = kernel.decompression_stream().unwrap().unwrap();
		let mut decompressed = Vec::new();
		std::io::Read::read_to_end(&mut stream, &mut decompressed).unwrap();

		assert!(decompressed.is_empty());
	}

	// Random data
	static FAKE_KERNEL_BYTES: &[u8] = include_bytes!("../test-data.bin");
	// Highly compressable data
	static FAKE_KERNEL_BYTES_MIN: &[u8] = include_bytes!("../test-data-min.bin");

	#[cfg(all(
		feature = "encoder",
		feature = "decoder",
		feature = "compression",
		feature = "decompression"
	))]
	#[test]
	fn roundtrip_minimal_kernel_uncompressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let mut kernel_cursor = std::io::Cursor::new(FAKE_KERNEL_BYTES_MIN);
		encoder
			.write_item(crate::item::KernelEncoder::new_uncompressed(
				&mut kernel_cursor,
				crate::item::KernelArch::Riscv64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);

		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, false);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Riscv64);

		assert_eq!(kernel.data.as_ref(), FAKE_KERNEL_BYTES_MIN);
	}

	#[cfg(all(feature = "encoder", feature = "decoder"))]
	#[test]
	fn roundtrip_minimal_kernel_compressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let mut kernel_cursor = std::io::Cursor::new(FAKE_KERNEL_BYTES_MIN);
		encoder
			.write_item(crate::item::KernelEncoder::new_compress_with_zstd(
				&mut kernel_cursor,
				3,
				crate::item::KernelArch::Aarch64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);
		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, true);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Aarch64);

		let mut stream = kernel.decompression_stream().unwrap().unwrap();
		let mut decompressed = Vec::<u8>::new();
		std::io::Read::read_to_end(&mut stream, &mut decompressed).unwrap();

		assert_eq!(decompressed, FAKE_KERNEL_BYTES_MIN);
	}

	#[cfg(all(
		feature = "encoder",
		feature = "decoder",
		feature = "compression",
		feature = "decompression"
	))]
	#[test]
	fn roundtrip_random_kernel_uncompressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let mut kernel_cursor = std::io::Cursor::new(FAKE_KERNEL_BYTES);
		encoder
			.write_item(crate::item::KernelEncoder::new_uncompressed(
				&mut kernel_cursor,
				crate::item::KernelArch::Riscv64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);

		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, false);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Riscv64);

		assert_eq!(kernel.data.as_ref(), FAKE_KERNEL_BYTES);
	}

	#[cfg(all(feature = "encoder", feature = "decoder"))]
	#[test]
	fn roundtrip_random_kernel_compressed() {
		let mut out = Vec::new();
		let mut cursor = std::io::Cursor::new(&mut out);
		let mut encoder = Encoder::new(&mut cursor).unwrap();
		let mut kernel_cursor = std::io::Cursor::new(FAKE_KERNEL_BYTES);
		encoder
			.write_item(crate::item::KernelEncoder::new_compress_with_zstd(
				&mut kernel_cursor,
				3,
				crate::item::KernelArch::Aarch64,
			))
			.unwrap();
		let written = encoder.finish().unwrap();
		assert!(written > 0);

		std::fs::write("/tmp/oro_ramdisk.bin", &out).unwrap();

		let decoder = Decoder::new(&out).unwrap();
		assert_eq!(decoder.index_items().len(), 1);
		let kernel = decoder.item::<crate::item::Kernel>(0).unwrap();
		assert_eq!(kernel.flags.compressed, true);
		assert_eq!(kernel.flags.arch, crate::item::KernelArch::Aarch64);

		let mut stream = kernel.decompression_stream().unwrap().unwrap();
		let mut decompressed = Vec::<u8>::new();
		std::io::Read::read_to_end(&mut stream, &mut decompressed).unwrap();

		assert_eq!(decompressed, FAKE_KERNEL_BYTES);
	}
}
