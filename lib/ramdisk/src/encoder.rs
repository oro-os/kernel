//! Encoder for the Oro Ramdisk format.

use std::io::{Seek, SeekFrom, Write};

use crate::{
	BigEndian, EncoderError, Header,
	item::{ItemDescriptor, ItemEncoder},
};

/// Encoder for the Oro ramdisk format.
///
/// This is only available for `std`-enabled builds, as it requires heap allocation.
pub struct Encoder<'a, W: Write + Seek + 'a> {
	writer: &'a mut W,
	header_pos: u64,
	ramdisk_crc64: oro_crc64::Crc64,
	index_items: Vec<crate::item::IndexItem>,
	total_written: u64,
}

impl<'a, W: Write + Seek> Encoder<'a, W> {
	/// Creates a new encoder for the given writer.
	pub fn new(writer: &'a mut W) -> Result<Self, EncoderError> {
		let header_pos = writer.stream_position()?;

		let mut s = Self {
			writer,
			header_pos,
			ramdisk_crc64: oro_crc64::Crc64::new(),
			index_items: Vec::new(),
			total_written: 0,
		};

		// Write a placeholder header, which will be updated later.
		// The placeholder header is set to all `0xFF` bytes, required for
		// proper CRC64 calculation of the entire ramdisk file upon decoding.
		s.write_bytes(&[0xFFu8; core::mem::size_of::<Header>()])?;

		Ok(s)
	}

	/// Aligns the stream to 8 bytes. Note that this is relative to the written
	/// bytes, not the stream position; the encoder doesn't care if the stream
	/// has had data written prior to this, and only cares about what shows up in memory.
	fn align(&mut self) -> Result<(), EncoderError> {
		static PADDING_BYTES: [u8; 7] = [0xFF; 7];

		let to_write = 8 - (self.total_written % 8);

		debug_assert_ne!(to_write, 0);

		if to_write < 8 {
			self.write_bytes(&PADDING_BYTES[..to_write as usize])?;
		}

		Ok(())
	}

	/// Writes a slice to the underlying writer, and updates the ramdisk CRC64 checksum.
	fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), EncoderError> {
		if bytes.is_empty() {
			return Ok(());
		}

		self.writer.write_all(bytes)?;
		self.ramdisk_crc64.update(bytes);
		let written =
			u64::try_from(bytes.len()).map_err(|_| crate::Error::RamdiskLengthMismatch)?;
		self.total_written = self
			.total_written
			.checked_add(written)
			.ok_or(crate::Error::RamdiskLengthMismatch)?;
		Ok(())
	}

	/// Writes a reader to the underlying writer, and updates the ramdisk CRC64 checksum.
	fn write_reader<R: std::io::Read>(
		&mut self,
		mut reader: R,
	) -> Result<(usize, u64), EncoderError> {
		let mut buffer = [0u8; 1024 * 64];
		let mut total = 0;
		let mut item_crc = oro_crc64::Crc64::new();

		loop {
			let bytes_read = reader.read(&mut buffer)?;
			if bytes_read == 0 {
				break;
			}
			total += bytes_read;
			item_crc.update(&buffer[..bytes_read]);
			self.write_bytes(&buffer[..bytes_read])?;
		}

		Ok((total, item_crc.finalize()))
	}

	/// Writes an item to the ramdisk, and adds it to the index.
	///
	/// # Edge case
	/// In the extremely off chance that the item is larger than `u64::MAX` bytes, this will return
	/// `RamdiskLengthMismatch` as the length cannot be represented in the index and will never
	/// be readable by the decoder anyway.
	///
	/// Note that this is _not_ the only case where `RamdiskLengthMismatch` can be returned; individual
	/// items might to return this error for various reasons, but typically not.
	pub fn write_item<I: ItemEncoder<'a>>(&mut self, item: I) -> Result<(u64, u64), EncoderError> {
		self.align()?;

		let item_offset = self.writer.stream_position()?;
		let (flags, mut reader) = item.encode()?;
		let (length, item_crc) = self.write_reader(&mut reader)?;

		let item_length = u64::try_from(length)
			.map_err(|_| EncoderError::Item(crate::Error::RamdiskLengthMismatch))?;

		let index_item = crate::item::IndexItem {
			tag: I::Descriptor::TAG,
			flags: BigEndian::from(flags.try_into().map_err(EncoderError::Item)?),
			offset: BigEndian::from(item_offset),
			length: BigEndian::from(item_length),
			crc: BigEndian::from(item_crc),
		};

		self.index_items.push(index_item);

		Ok((item_offset, item_length))
	}

	/// Finalizes the encoder; writes the index and populates the header.
	///
	/// Returns the final length of data written.
	pub fn finish(mut self) -> Result<u64, EncoderError> {
		// Write the index.
		let index_items = core::mem::take(&mut self.index_items);
		let (index_offset, index_length) =
			self.write_item(crate::item::IndexEncoder::new(index_items)?)?;

		// Construct the final header.
		let mut ramdisk_header = Header {
			magic: Header::MAGIC,
			total_length: BigEndian::from(self.total_written),
			header_crc: BigEndian::from(0),
			ramdisk_crc: BigEndian::from(self.ramdisk_crc64.finalize()),
			index_offset: BigEndian::from(index_offset),
			index_length: BigEndian::from(index_length),
			reserved: [0u8; _],
			ramdisk_start: [0u8; 0],
		};

		let crc = ramdisk_header.calculate_header_crc();
		ramdisk_header.header_crc = BigEndian::from(crc);

		let original_cursor = self.writer.stream_position()?;
		self.writer.seek(SeekFrom::Start(self.header_pos))?;
		self.writer.write_all(Header::as_bytes(&ramdisk_header))?;
		self.writer.seek(SeekFrom::Start(original_cursor))?;

		Ok(self.total_written)
	}
}
