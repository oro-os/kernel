//! Error type for the ramdisk library.

/// Error type for the ramdisk library.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Error {
	/// The root ramdisk pointer is not aligned to 8 bytes.
	UnalignedRootPointer,
	/// The ramdisk header contains an invalid magic number.
	InvalidRamdiskMagic([u8; 8]),
	/// The ramdisk header contains an invalid CRC64 checksum for the main header.
	InvalidRamdiskHeaderCrc,
	/// The ramdisk header contains an invalid CRC64 checksum for the entire ramdisk.
	InvalidRamdiskCrc,
	/// The ramdisk header contains invalid flags.
	InvalidRamdiskHeaderFlags,
	/// Reserved space is not zeroed.
	ReservedSpaceNotZeroed,
	/// The header specified a byte length that did not match
	/// the given slice's length, or the total length of the
	/// ramdisk exceeds the platform's pointer width.
	RamdiskLengthMismatch,
	/// The ramdisk header gave an index that was either out of bounds
	/// for the ramdisk, or that exceeded the pointer width of the
	/// underlying system.
	RamdiskIndexOutOfBounds,
	/// The ramdisk index is not aligned to 8 bytes.
	UnalignedIndex,
	/// The ramdisk's CRC64 validation failed.
	InvalidIndexCrc,
	/// An index item was found that was not aligned to 8 bytes.
	UnalignedIndexItem(usize),
	/// An index item was found that was out of bounds for the ramdisk.
	IndexItemOutOfBounds(usize),
	/// The index blob did not start with the expected root index tag.
	InvalidRootIndexTag([u8; 4]),
	/// While fetching an item's data from the ramdisk, the item's declared
	/// tag in the index did not match the request tag.
	InvalidItemTag { expected: [u8; 4], found: [u8; 4] },
	/// The index item was too small to contain the requested item type.
	IndexItemTooSmall {
		min_length: usize,
		found_length: usize,
	},
	/// The index item's descriptor in the index specified a CRC64 checksum that did not match the item's data.
	InvalidItemCrc { expected: u64, found: u64 },
	/// The reserved parts of the index item's flags were not zeroed. This may
	/// either be some or all of the flags.
	ReservedFlagsNotZero(u32),
	/// The specified architecture for the kernel item is not supported by this build of the ramdisk library.
	UnsupportedKernelArch(u8),
}

/// Error type returned by [`crate::RamdiskDecoder::item`] when the item is invalid.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ItemError {
	/// Fetching the item itself failed, e.g. the index item was out of bounds or unaligned.
	Fetch(usize, Error),
	/// The item was fetched, but the item's flag validation failed.
	Flags(usize, Error),
	/// The item was fetched, but the per-item-type validation failed.
	Validation(usize, Error),
}

/// An error that is returned by [`Encoder::write_item`].
#[cfg(feature = "encoder")]
#[derive(Debug)]
pub enum EncoderError {
	/// The item failed to encode itself. This is a ramdisk-specific
	/// error, and is not related to I/O.
	Item(crate::Error),
	/// An I/O error occurred while writing the item to the underlying writer.
	Io(std::io::Error),
}

#[cfg(feature = "encoder")]
impl From<crate::Error> for EncoderError {
	fn from(e: crate::Error) -> Self {
		Self::Item(e)
	}
}

#[cfg(feature = "encoder")]
impl From<std::io::Error> for EncoderError {
	fn from(e: std::io::Error) -> Self {
		Self::Io(e)
	}
}
