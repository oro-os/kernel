//! Implements the Kernel data item.

/// A data item in the ramdisk that holds the Oro kernel binary.
///
/// The binary data itself is the raw kernel image, either compressed
/// or uncompressed. See [`KernelFlags`] for the flags stored in the index
/// and what they mean.
pub struct Kernel<'a> {
	/// The kernel binary data.
	pub data: &'a [u8],
	/// The flags for the kernel data.
	pub flags: KernelFlags,
}

impl Kernel<'_> {
	/// Returns a decompression stream over the kernel data, if it is compressed.
	///
	/// Returns `None` if the kernel data is not compressed.
	#[cfg(feature = "decompression")]
	pub fn decompression_stream(
		&self,
	) -> Option<
		Result<
			ruzstd::decoding::StreamingDecoder<&[u8], ruzstd::decoding::FrameDecoder>,
			ruzstd::decoding::errors::FrameDecoderError,
		>,
	> {
		if self.flags.compressed {
			Some(ruzstd::decoding::StreamingDecoder::new(self.data))
		} else {
			None
		}
	}
}

impl super::ItemDescriptor for Kernel<'_> {
	type Flags = KernelFlags;
	type Output<'a> = Kernel<'a>;

	/// The minimum length of the kernel item.
	const MIN_LENGTH: usize = 0;
	/// The tag of the kernel item.
	const TAG: [u8; 4] = *b"KERN";

	fn from_slice<'a>(flags: Self::Flags, slice: &'a [u8]) -> Self::Output<'a> {
		Kernel { data: slice, flags }
	}
}

impl<'a> super::Item for Kernel<'a> {
	type Descriptor = Kernel<'a>;

	fn validate(&self) -> Result<(), crate::Error> {
		// The kernel item has no additional validation requirements.
		Ok(())
	}
}

impl super::private::Sealed for Kernel<'_> {}

/// Encoder for kernel items.
#[cfg(feature = "encoder")]
pub struct KernelEncoder<R: std::io::Read> {
	/// The kernel binary data, uncompressed.
	data: R,
	/// Whether or not the kernel data is compressed,
	/// or should be compressed.
	compressed: KernelEncoderCompression,
	/// The architecture for the kernel data.
	arch: KernelArch,
}

#[cfg(feature = "encoder")]
enum KernelEncoderCompression {
	/// The kernel data is uncompressed.
	Uncompressed,
	/// The kernel data is compressed using the ZSTD algorithm.
	Compressed,
	/// The kernel data is uncompressed, but should be compressed using the ZSTD algorithm.
	#[cfg(feature = "compression")]
	Compress(i32),
}

#[cfg(feature = "encoder")]
impl<R: std::io::Read> KernelEncoder<R> {
	/// Creates a new kernel encoder with uncompressed data.
	///
	/// The data will **not** be compressed when written to the ramdisk.
	pub fn new_uncompressed(data: R, arch: KernelArch) -> Self {
		Self {
			data,
			compressed: KernelEncoderCompression::Uncompressed,
			arch,
		}
	}

	/// Creates a new kernel encoder with compressed data.
	///
	/// **The data must already be ZSTD compressed**. The encoder will not compress the data again,
	/// and the ramdisk will not be able to decompress it if it is not valid ZSTD compressed data.
	pub fn new_with_already_zstd_compressed(data: R, arch: KernelArch) -> Self {
		Self {
			data,
			compressed: KernelEncoderCompression::Compressed,
			arch,
		}
	}

	/// Creates a new kernel encoder with uncompressed data that will be compressed when written to the ramdisk.
	///
	/// Uses the given `level` for the ZSTD compression.
	///
	/// This requires the `compression` feature to be enabled.
	///
	/// # Panics
	/// Panics if level is not in the range `-7..=22`.
	#[cfg(feature = "compression")]
	pub fn new_compress_with_zstd(data: R, level: i32, arch: KernelArch) -> Self {
		Self {
			data,
			compressed: KernelEncoderCompression::Compress(level),
			arch,
		}
	}
}

#[cfg(feature = "encoder")]
impl<'a, R: std::io::Read + 'a> super::ItemEncoder<'a> for KernelEncoder<R> {
	type Descriptor = Kernel<'a>;

	fn encode(
		self,
	) -> Result<
		(
			<Self::Descriptor as super::ItemDescriptor>::Flags,
			impl std::io::Read + 'a,
		),
		crate::EncoderError,
	> {
		let (flags, reader): (KernelFlags, Box<dyn std::io::Read + 'a>) = match self.compressed {
			KernelEncoderCompression::Uncompressed => {
				let flags = KernelFlags {
					compressed: false,
					arch: self.arch,
				};
				(flags, Box::new(self.data) as Box<dyn std::io::Read + 'a>)
			}
			KernelEncoderCompression::Compressed => {
				let flags = KernelFlags {
					compressed: true,
					arch: self.arch,
				};
				(flags, Box::new(self.data) as Box<dyn std::io::Read + 'a>)
			}
			#[cfg(feature = "compression")]
			KernelEncoderCompression::Compress(level) => {
				let flags = KernelFlags {
					compressed: true,
					arch: self.arch,
				};
				(
					flags,
					Box::new(zstd::stream::read::Encoder::new(self.data, level)?)
						as Box<dyn std::io::Read + 'a>,
				)
			}
		};

		Ok((flags, reader))
	}
}

/// The flags for the kernel data
///
/// | Bit | Name | Description |
/// |-----|------|-------------|
/// | 0   | Compressed | Whether or not the kernel is compressed using the ZSTD algorithm. |
/// | 1-23 | Reserved | Must be zero. |
/// | 24-31 | Arch | The architecture for this kernel image (see [`KernelArch`]). |
pub struct KernelFlags {
	/// Whether or not the kernel is compressed using the ZSTD algorithm.
	pub compressed: bool,
	/// The architecture for this kernel image.
	pub arch: KernelArch,
}

impl TryFrom<u32> for KernelFlags {
	type Error = crate::Error;

	fn try_from(flags: u32) -> Result<Self, Self::Error> {
		let compressed = (flags & 0x1) != 0;
		let arch = KernelArch::try_from(((flags >> 24) & 0xFF) as u8)?;

		if flags & 0xFFFFFE != 0 {
			return Err(crate::Error::ReservedFlagsNotZero(flags));
		}

		Ok(KernelFlags { compressed, arch })
	}
}

impl TryFrom<KernelFlags> for u32 {
	type Error = crate::Error;

	fn try_from(flags: KernelFlags) -> Result<Self, Self::Error> {
		let mut value = 0u32;
		if flags.compressed {
			value |= 0x1;
		}
		value |= (u32::from(flags.arch as u8) & 0xFF) << 24;
		Ok(value)
	}
}

/// The supported architectures for the kernel image.
///
/// The discriminants are **stable**, meaning that they will
/// not change between versions of the ramdisk format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum KernelArch {
	// DO NOT CHANGE THE DISCRIMINANTS OF THESE ENUM VARIANTS.
	//
	// Either ADD or REMOVE them.
	//
	// When REMOVING, **comment it out**. Do NOT delete it.
	//
	// This allows future maintainers to see which discriminants were used in the past,
	// and prevents accidental reuse of a discriminant.
	//
	// The kernel is invalid or not specified. This has always been commented out,
	// but is documented here for completeness. It is not nor has it ever been an explicit
	// check, but has always been reserved to mean 'invalid'.
	// INVALID = 0,
	/// The kernel is for the x86_64 architecture.
	X86_64 = 1,
	/// The kernel is for the aarch64 architecture.
	Aarch64 = 2,
	/// The kernel is for the riscv64 architecture.
	Riscv64 = 3,
}

impl TryFrom<u8> for KernelArch {
	type Error = crate::Error;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			1 => Ok(KernelArch::X86_64),
			2 => Ok(KernelArch::Aarch64),
			3 => Ok(KernelArch::Riscv64),
			value => Err(crate::Error::UnsupportedKernelArch(value)),
		}
	}
}

impl From<KernelArch> for u8 {
	fn from(arch: KernelArch) -> Self {
		arch as u8
	}
}
