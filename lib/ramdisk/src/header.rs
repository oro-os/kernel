//! Header type for the ramdisk format.

use core::ops::Range;

use crate::{BigEndian, Error};

/// Ramdisk header.
///
/// The magic number identifying this ramdisk format is `4F 52 4F 44 53 4B 30 30` (`ORODSK00`).
///
/// The header may be extended in the future; as such, the `index_offset`
/// is used to locate the index block, which is the root of the ramdisk's object tree,
/// instead of assuming that the index block is located immediately after the header.
///
/// | Field | Type | Offset | Size | Description | Notes |
/// |---|---|---|---|---|---|
/// | `magic` | magic | 0 | 8 | Magic number | always `'ORODSK00'` (`4F 52 4F 44 53 4B 30 30`) |
/// | `total_length` | `u64` | 8 | 8 | Total length of the ramdisk file, including header | big-endian |
/// | `header_crc` | `u64` | 16 | 8 | A CRC64 checksum of the header contents, including reserved region. | zero this field before performing header CRC/verification |
/// | `ramdisk_crc` | `u64` | 24 | 8 | A CRC64 checksum of the entire ramdisk file, with the header of all `0xFF` bytes. | zero this field before performing header CRC/verification |
/// | `index_offset` | `u64` | 32 | 8 | The byte offset to the index root | indices start from ramdisk start (magic number starts at index `0`) |
/// | `index_length` | `u64` | 40 | 8 | The length of the index block, in bytes | big-endian |
/// | `reserved` | reserved | 48 | 16 | Reserved for future use | currently reserved |
#[derive(Debug, Clone)]
#[repr(C, align(8))]
pub struct Header {
	/// Magic number identifying the file as a ramdisk, including the version.
	pub(crate) magic: [u8; 8],
	/// Total length of the ramdisk file, including the header.
	pub(crate) total_length: BigEndian<u64>,
	/// CRC64 checksum of the header contents, including the reserved region.
	pub(crate) header_crc: BigEndian<u64>,
	/// CRC64 checksum of the entire ramdisk file, including the header.
	pub(crate) ramdisk_crc: BigEndian<u64>,
	/// Byte offset to the index root, which is the root of the ramdisk's object tree.
	pub(crate) index_offset: BigEndian<u64>,
	/// The length of the index block, in bytes.
	pub(crate) index_length: BigEndian<u64>,
	/// Reserved for future use; must be 0.
	pub(crate) reserved: [u8; 16],
	/// The start of the rest of the ramdisk.
	pub(crate) ramdisk_start: [u8; 0],
}

const _: () = assert!(core::mem::size_of::<Header>() == 64);

impl Header {
	/// Magic number identifying this ramdisk format.
	pub const MAGIC: [u8; 8] = *b"ORODSK00";

	/// Returns the header version encoded in the magic number.
	pub fn version(&self) -> u8 {
		0
	}

	/// Returns the total length of the ramdisk file, including the header.
	pub fn total_length(&self) -> u64 {
		self.total_length.to_ne()
	}

	/// Returns the byte offset to the root index.
	pub fn index_offset(&self) -> u64 {
		self.index_offset.to_ne()
	}

	/// Returns the length of the root index, in bytes.
	pub fn index_length(&self) -> u64 {
		self.index_length.to_ne()
	}

	#[allow(unused)]
	pub(crate) fn validate(&self, data: &[u8]) -> Result<(), Error> {
		if self.magic != Self::MAGIC {
			return Err(Error::InvalidRamdiskMagic(self.magic));
		}

		self.verify_header_crc()?;

		let total_length = self.total_length_usize()?;
		if total_length < core::mem::size_of::<Self>() || total_length != data.len() {
			return Err(Error::RamdiskLengthMismatch);
		}

		self.index_bounds()?;

		if !self.reserved.iter().all(|&b| b == 0) {
			return Err(Error::ReservedSpaceNotZeroed);
		}

		self.verify_ramdisk_crc(data)?;

		Ok(())
	}

	#[allow(unused)]
	pub(crate) fn index_bounds(&self) -> Result<Range<usize>, Error> {
		let total_length = self.total_length_usize()?;
		let index_offset =
			usize::try_from(self.index_offset()).map_err(|_| Error::RamdiskIndexOutOfBounds)?;
		let index_length =
			usize::try_from(self.index_length()).map_err(|_| Error::RamdiskIndexOutOfBounds)?;

		if index_offset > total_length || index_length > total_length {
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		let index_end = index_offset
			.checked_add(index_length)
			.ok_or(Error::RamdiskIndexOutOfBounds)?;
		if index_end > total_length {
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		Ok(index_offset..index_end)
	}

	fn total_length_usize(&self) -> Result<usize, Error> {
		usize::try_from(self.total_length()).map_err(|_| Error::RamdiskLengthMismatch)
	}

	/// Performs the CRC64 checksum of the header contents, including the reserved region.
	pub fn calculate_header_crc(&self) -> u64 {
		let mut header = self.clone();
		header.header_crc = BigEndian::from(0);
		header.ramdisk_crc = BigEndian::from(0);

		oro_crc64::Crc64::checksum(Self::as_bytes(&header))
	}

	/// Verifies the CRC64 checksum of the header contents, including the reserved region.
	pub fn verify_header_crc(&self) -> Result<(), Error> {
		if self.calculate_header_crc() != self.header_crc.to_ne() {
			return Err(Error::InvalidRamdiskHeaderCrc);
		}

		Ok(())
	}

	/// Performs the CRC64 checksum of the entire ramdisk file, including the header.
	pub fn calculate_ramdisk_crc(&self, data: &[u8]) -> Result<u64, Error> {
		let total_length = self.total_length_usize()?;
		if total_length < core::mem::size_of::<Self>() || total_length != data.len() {
			return Err(Error::RamdiskLengthMismatch);
		}

		// The header itself is CRC'd with the header set to all `0xFF`s.
		let mut crc = oro_crc64::Crc64::new();
		crc.update([0xFFu8; core::mem::size_of::<Self>()].as_ref());
		crc.update(
			data.get(core::mem::size_of::<Self>()..)
				.ok_or(Error::RamdiskLengthMismatch)?,
		);

		Ok(crc.finalize())
	}

	/// Verifies the CRC64 checksum of the entire ramdisk file, including the header.
	pub fn verify_ramdisk_crc(&self, data: &[u8]) -> Result<(), Error> {
		if self.calculate_ramdisk_crc(data)? != self.ramdisk_crc.to_ne() {
			return Err(Error::InvalidRamdiskCrc);
		}
		Ok(())
	}

	pub(crate) fn as_bytes(&self) -> &[u8] {
		// SAFETY: `header` is a valid reference, and any initialized Rust value may be viewed as bytes.
		unsafe {
			core::slice::from_raw_parts(
				core::ptr::from_ref(self).cast::<u8>(),
				core::mem::size_of::<Self>(),
			)
		}
	}
}

impl Default for Header {
	fn default() -> Self {
		Self {
			magic: Self::MAGIC,
			total_length: BigEndian::from(0),
			header_crc: BigEndian::from(0),
			ramdisk_crc: BigEndian::from(0),
			index_offset: BigEndian::from(0),
			index_length: BigEndian::from(0),
			reserved: [0; 16],
			ramdisk_start: [],
		}
	}
}
