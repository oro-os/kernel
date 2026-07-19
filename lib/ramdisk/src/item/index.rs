//! Implements the root index object (tag `'ROOT'`) for the ramdisk file format.
//!
//! See the crate root documentation for more information on the ramdisk file format and its structure,
//! including this index object and its items.

use crate::{BigEndian, Error};

/// The root index object.
///
/// The index block is the 'root' of the ramdisk's object tree. The object tree
/// is a flat list of objects that are stored in the ramdisk, and is used to locate objects
/// within the file.
///
/// The index does not need to contain itself as an item, though it's not an error if it does.
/// Further, the index does not need to be the first object in the ramdisk; in fact, most ramdisks
/// will likely have the index at the end of the file.
///
/// | Field | Type | Offset | Size | Description | Notes |
/// |---|---|---|---|---|---|
/// | `tag` | `'52 4F 4F 54'` | 0 | 4 | Magic number | always `'ROOT'` (`52 4F 4F 54`) |
/// | `count` | `u32` | 4 | 4 | The number of items in the index |  |
/// | `crc64` | `u64` | 8 | 8 | A CRC64 checksum of the index contents, including reserved region. | zero this field before performing CRC/verification |
/// | `flags` | reserved | 16 | 16 | Object flags | reserved |
/// | `items...` | Index Item | 32 | `count * sizeof(Index Item)` | The items in the ramdisk |  |
#[derive(Debug, Clone)]
#[repr(C, align(8))]
pub struct Index {
	/// The magic number identifying this object as the root index (`'ROOT'`).
	pub tag: [u8; 4],
	/// The number of items in the index.
	pub count: BigEndian<u32>,
	/// The CRC64 checksum of the index contents.
	pub crc64: BigEndian<u64>,
	/// Reserved for future use.
	pub reserved: [u8; 16],
	/// The items in the index.
	pub items: [IndexItem; 0],
}

const _: () = assert!(core::mem::size_of::<Index>() == 32);

/// An item in the root index.
///
/// Each index item is a single entry in the index block, starting from `0` and ending at `count - 1`.
/// Certain block types will refer to other items in the ramdisk by their index, which is the position
/// of the item in this index block.
///
/// Some item types may be singletons, meaning that there may only be one item of that type in the ramdisk.
/// Other item types may be repeated, meaning that there may be multiple items of that type in the ramdisk.
///
/// | Field | Type | Offset | Size | Description | Notes |
/// |---|---|---|---|---|---|
/// | `type` | `[u8; 4]` | 0 | 4 | The object's tag | the `tag` field for each item type |
/// | `flags` | reserved | 4 | 4 | Index item flags | see each item's `flags` documentation |
/// | `offset` | `u64` | 8 | 8 | The byte offset of the start of this object | indices start from the ramdisk start (magic number starts at index `0`) |
/// | `length` | `u64` | 16 | 8 | The total number of bytes in the object |  |
#[derive(Debug, Clone)]
#[repr(C, align(8))]
pub struct IndexItem {
	/// The tag identifying the type of this item.
	pub tag: [u8; 4],
	/// Reserved flags for future use.
	pub flags: BigEndian<u32>,
	/// The byte offset of the start of this object.
	pub offset: BigEndian<u64>,
	/// The total number of bytes in this object.
	pub length: BigEndian<u64>,
	/// The CRC64 checksum of this object's data.
	pub crc: BigEndian<u64>,
}

const _: () = assert!(core::mem::size_of::<IndexItem>() == 32);

impl Index {
	/// The tag of the root index object.
	pub const TAG: [u8; 4] = *b"ROOT";

	/// Returns the total byte length of the index, including the header
	/// and all items.
	pub fn total_length(&self) -> Result<usize, Error> {
		usize::try_from(self.count.to_ne())
			.map_err(|_| Error::RamdiskIndexOutOfBounds)?
			.checked_mul(core::mem::size_of::<IndexItem>())
			.and_then(|s| core::mem::size_of::<Index>().checked_add(s))
			.ok_or(Error::RamdiskIndexOutOfBounds)
	}

	/// Validates the CRC64 checksum of the index.
	pub fn validate_crc(&self, items: &[IndexItem]) -> Result<(), Error> {
		if self.calculate_crc(items)? != self.crc64.to_ne() {
			return Err(Error::InvalidIndexCrc);
		}

		Ok(())
	}

	/// Calculates the CRC64 checksum of the index, including the header and all items.
	pub fn calculate_crc(&self, items: &[IndexItem]) -> Result<u64, Error> {
		if usize::try_from(self.count.to_ne()).map_err(|_| Error::RamdiskIndexOutOfBounds)?
			!= items.len()
		{
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		let mut index = self.clone();
		index.crc64 = BigEndian::from(0);
		let mut crc = oro_crc64::Crc64::new();

		// First calculate the CRC of the index header (without the items).
		crc.update(index.as_bytes());

		// Then calculate the CRC of the index items.
		// SAFETY: `items` is a valid slice, and any initialized Rust value may be viewed as bytes.
		let index_items = unsafe {
			core::slice::from_raw_parts(items.as_ptr().cast::<u8>(), core::mem::size_of_val(items))
		};
		crc.update(index_items);

		Ok(crc.finalize())
	}

	/// Returns `self` as a byte slice.
	fn as_bytes(&self) -> &[u8] {
		// SAFETY: Always safe
		unsafe {
			core::slice::from_raw_parts(
				core::ptr::from_ref::<Self>(self).cast::<u8>(),
				core::mem::size_of::<Self>(),
			)
		}
	}
}

#[cfg(feature = "encoder")]
impl IndexItem {
	/// Returns `self` as a byte slice.
	fn as_bytes(&self) -> &[u8] {
		// SAFETY: Always safe
		unsafe {
			core::slice::from_raw_parts(
				core::ptr::from_ref::<Self>(self).cast::<u8>(),
				core::mem::size_of::<Self>(),
			)
		}
	}
}

impl super::ItemDescriptor for Index {
	type Flags = super::ReservedFlags;
	type Output<'a> = &'a Self;

	const MIN_LENGTH: usize = core::mem::size_of::<Self>();
	const TAG: [u8; 4] = Self::TAG;

	fn from_slice<'a>(_flags: Self::Flags, slice: &'a [u8]) -> Self::Output<'a> {
		// SAFETY: The slice is guaranteed to be at least the size of the index header,
		// and the index header is aligned to 8 bytes.
		unsafe { &*(slice.as_ptr().cast::<Self>()) }
	}
}

impl super::Item for &Index {
	type Descriptor = Index;

	fn validate(&self) -> Result<(), Error> {
		// Note that we've already validated it at the decoder level;
		// we don't need to do anything here as it's redundant checking.
		Ok(())
	}
}

impl super::private::Sealed for Index {}

/// The encoder for the root index object.
#[cfg(feature = "encoder")]
pub struct IndexEncoder {
	index_items: Vec<IndexItem>,
	index: Index,
}

#[cfg(feature = "encoder")]
impl IndexEncoder {
	/// Creates a new index encoder with the given index items.
	pub fn new(index_items: Vec<IndexItem>) -> Result<Self, Error> {
		let mut index = Index {
			tag: Index::TAG,
			count: BigEndian::from(
				u32::try_from(index_items.len())
					.map_err(|_| crate::Error::RamdiskIndexOutOfBounds)?,
			),
			crc64: BigEndian::from(0),
			reserved: [0u8; 16],
			items: [],
		};

		let crc = index.calculate_crc(&index_items)?;

		index.crc64 = BigEndian::from(crc);

		Ok(Self { index, index_items })
	}
}

#[cfg(feature = "encoder")]
impl<'a> super::ItemEncoder<'a> for IndexEncoder {
	type Descriptor = Index;

	fn encode(
		self,
	) -> Result<
		(
			<Self::Descriptor as super::ItemDescriptor>::Flags,
			impl std::io::Read + 'a,
		),
		crate::EncoderError,
	> {
		Ok((
			super::ReservedFlags,
			crate::chunked_stream::async_read_generator(async move |mut w| {
				w.write(self.index.as_bytes()).await;

				for item in self.index_items {
					w.write(item.as_bytes()).await;
				}
			}),
		))
	}
}
