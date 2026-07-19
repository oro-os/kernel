//! All items implemented by the ramdisk format.

mod index;
mod kernel;

pub use self::{index::*, kernel::*};

pub(crate) mod private {
	pub trait Sealed {}
}

/// Represents an item descirptor (type) in the ramdisk format.
pub trait ItemDescriptor: private::Sealed {
	/// The item's tag.
	const TAG: [u8; 4];

	/// The item's minimum length.
	const MIN_LENGTH: usize;

	/// The output of [`Self::from_slice`].
	type Output<'a>: Item + 'a;

	/// The flags of the item, if any.
	type Flags: TryFrom<u32, Error = crate::Error> + TryInto<u32, Error = crate::Error>;

	/// Converts a slice of bytes into the item.
	///
	/// This may or may not be a projection of the slice,
	/// or a wrapper type that borrows the slice.
	fn from_slice<'a>(flags: Self::Flags, slice: &'a [u8]) -> Self::Output<'a>;
}

/// Represents an actual item fetched from the ramdisk as an output of [`ItemDescriptor`].
pub trait Item {
	/// The item descriptor type that this item corresponds to.
	type Descriptor: ItemDescriptor;

	/// Validates the item upon fetch.
	fn validate(&self) -> Result<(), crate::Error>;
}

/// Represents an encoder for an item.
///
/// Encoders are structures used to parameterize encoding items into
/// a Ramdisk file, and are different from the item types themselves
/// as they generally use read streams and other temporary, stack-based
/// values to encode.
#[cfg(feature = "encoder")]
pub trait ItemEncoder<'a> {
	/// The item descriptor type that this encoder corresponds to.
	type Descriptor: ItemDescriptor;

	/// Consumes the item, returning the encodable flags and a reader for the item data.
	fn encode(
		self,
	) -> Result<
		(
			<Self::Descriptor as ItemDescriptor>::Flags,
			impl std::io::Read + 'a,
		),
		crate::EncoderError,
	>;
}

/// Default flags type for items that do not have any flags; specifies
/// that the flags are reserved and must be zero.
pub struct ReservedFlags;

impl TryFrom<u32> for ReservedFlags {
	type Error = crate::Error;

	fn try_from(flags: u32) -> Result<Self, Self::Error> {
		if flags != 0 {
			return Err(crate::Error::ReservedFlagsNotZero(flags));
		}
		Ok(ReservedFlags)
	}
}

impl TryFrom<ReservedFlags> for u32 {
	type Error = crate::Error;

	fn try_from(_flags: ReservedFlags) -> Result<Self, Self::Error> {
		Ok(0)
	}
}
