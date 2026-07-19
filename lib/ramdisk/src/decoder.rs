//! Decoder for the ramdisk file format.

use crate::{
	Error, Header, ItemError,
	item::{Index, IndexItem, Item, ItemDescriptor},
};

pub struct Decoder<'a> {
	data: &'a [u8],
	header: &'a Header,
	index: &'a Index,
	index_items: &'a [IndexItem],
}

impl<'a> Decoder<'a> {
	/// Creates a new decoder for a bounded ramdisk byte slice.
	pub fn new(data: &'a [u8]) -> Result<Self, Error> {
		if data.len() < core::mem::size_of::<Header>() {
			return Err(Error::RamdiskLengthMismatch);
		}

		let header_ptr = data.as_ptr().cast::<Header>();
		if !header_ptr.is_aligned() {
			return Err(Error::UnalignedRootPointer);
		}

		// SAFETY: The pointer is derived from `data`, the slice is large enough for `Header`,
		// SAFETY: and the alignment was checked above. `Header` accepts all byte patterns.
		let header = unsafe { &*header_ptr };
		header.validate(data)?;

		let index_bounds = header.index_bounds()?;
		let index_bytes = data
			.get(index_bounds)
			.ok_or(Error::RamdiskIndexOutOfBounds)?;
		if index_bytes.len() < core::mem::size_of::<Index>() {
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		let index_ptr = index_bytes.as_ptr().cast::<Index>();
		if !index_ptr.is_aligned() {
			return Err(Error::UnalignedIndex);
		}

		// SAFETY: The pointer is derived from `data`, the index slice is large enough for `Index`,
		// SAFETY: and the alignment was checked above. `Index` accepts all byte patterns.
		let index = unsafe { &*index_ptr };

		if index.tag != Index::TAG {
			return Err(Error::InvalidRootIndexTag(index.tag));
		}

		let total_index_length = index.total_length()?;
		if total_index_length != index_bytes.len() {
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		let index_item_count =
			usize::try_from(index.count.to_ne()).map_err(|_| Error::RamdiskIndexOutOfBounds)?;
		let index_items_length = index_item_count
			.checked_mul(core::mem::size_of::<IndexItem>())
			.ok_or(Error::RamdiskIndexOutOfBounds)?;
		let index_item_bytes = &index_bytes[core::mem::size_of::<Index>()..];
		if index_items_length != index_item_bytes.len() {
			return Err(Error::RamdiskIndexOutOfBounds);
		}

		// SAFETY: The item bytes are within `data`, their length was checked from `count`,
		// SAFETY: and the index header size preserves 8-byte alignment from the aligned index pointer.
		let index_items = unsafe {
			core::slice::from_raw_parts(
				index_item_bytes.as_ptr().cast::<IndexItem>(),
				index_item_count,
			)
		};

		index.validate_crc(index_items)?;
		Self::validate_index_items(header.total_length(), index_items)?;

		Ok(Self {
			data,
			header,
			index,
			index_items,
		})
	}

	/// Returns the full ramdisk byte slice.
	pub fn data(&self) -> &'a [u8] {
		self.data
	}

	/// Returns the validated ramdisk header.
	pub fn header(&self) -> &'a Header {
		self.header
	}

	/// Returns the validated root index.
	pub fn index(&self) -> &'a Index {
		self.index
	}

	/// Returns the validated root index items.
	pub fn index_items(&self) -> &'a [IndexItem] {
		self.index_items
	}

	fn validate_index_items(total_length: u64, items: &[IndexItem]) -> Result<(), Error> {
		for (i, item) in items.iter().enumerate() {
			let offset = item.offset.to_ne();
			let length = item.length.to_ne();

			if (offset % 8) != 0 {
				return Err(Error::UnalignedIndexItem(i));
			}

			if (length == 0 && offset > total_length) || (length > 0 && offset >= total_length) {
				return Err(Error::IndexItemOutOfBounds(i));
			}

			let end = offset
				.checked_add(length)
				.ok_or(Error::IndexItemOutOfBounds(i))?;

			if length != 0 && end > total_length {
				return Err(Error::IndexItemOutOfBounds(i));
			}
		}

		Ok(())
	}

	/// Returns the slice for the given index item, if it is valid.
	pub fn item_slice(&self, item: usize) -> Result<&'a [u8], Error> {
		let item_def = self
			.index_items
			.get(item)
			.ok_or(Error::IndexItemOutOfBounds(item))?;

		let offset = usize::try_from(item_def.offset.to_ne())
			.map_err(|_| Error::IndexItemOutOfBounds(item))?;
		let length = usize::try_from(item_def.length.to_ne())
			.map_err(|_| Error::IndexItemOutOfBounds(item))?;

		let end = offset
			.checked_add(length)
			.ok_or(Error::IndexItemOutOfBounds(item))?;
		self.data
			.get(offset..end)
			.ok_or(Error::IndexItemOutOfBounds(item))
	}

	/// Returns the reference to the given index item, if it is valid.
	///
	/// Performs validation of the item's blob against its tag, alignment,
	/// checksum, etc.
	pub fn item<T: ItemDescriptor>(&self, item: usize) -> Result<T::Output<'a>, ItemError> {
		let item_def = self
			.index_items
			.get(item)
			.ok_or(ItemError::Fetch(item, Error::IndexItemOutOfBounds(item)))?;

		if item_def.tag != T::TAG {
			return Err(ItemError::Fetch(
				item,
				Error::InvalidItemTag {
					expected: T::TAG,
					found: item_def.tag,
				},
			));
		}

		let item_slice = self
			.item_slice(item)
			.map_err(|err| ItemError::Fetch(item, err))?;

		if item_slice.len() < T::MIN_LENGTH {
			return Err(ItemError::Fetch(
				item,
				Error::IndexItemTooSmall {
					min_length: T::MIN_LENGTH,
					found_length: item_slice.len(),
				},
			));
		}

		let item_crc = item_def.crc.to_ne();
		let real_crc = oro_crc64::Crc64::checksum(item_slice);
		if item_crc != real_crc {
			return Err(ItemError::Fetch(
				item,
				Error::InvalidItemCrc {
					expected: item_crc,
					found: real_crc,
				},
			));
		}

		let flags = T::Flags::try_from(item_def.flags.to_ne())
			.map_err(|err| ItemError::Flags(item, err))?;

		let output = T::from_slice(flags, item_slice);
		output
			.validate()
			.map_err(|err| ItemError::Validation(item, err))?;

		Ok(output)
	}
}
