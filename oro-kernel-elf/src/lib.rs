//! ELF parser. This parser is quite bare-bones as it's only
//! needed for loading the kernel and modules and extracting
//! the module metadata.
#![cfg_attr(not(test), no_std)]
// SAFETY(qix-): This is approved and is to be merged soon.
// SAFETY(qix-): It's not used for anything critical, and the changes
// SAFETY(qix-): to the API after stabilization will be minor.
// SAFETY(qix-): https://github.com/rust-lang/rust/issues/117729
#![feature(debug_closure_helpers)]
#![cfg_attr(doc, feature(doc_cfg))]

use core::{fmt, mem::ManuallyDrop, ptr::from_ref};

use oro_kernel_macro::assert;

/// Marks a segment header as a kernel code segment.
const ORO_ELF_FLAGTYPE_KERNEL_CODE: u32 = 1 << 20;
/// Marks a segment header as the Oro boot protocol segment.
/// Can only be one.
const ORO_ELF_FLAGTYPE_KERNEL_BOOT_PROTOCOL: u32 = 1 << 21;

/// Main entrypoint for an ELF file.
///
/// It expects that the ELF file is already loaded into memory.
/// at a contiguous segment of memory.
///
/// # Safety
/// The ELF file must be loaded into memory and the base address
/// must be valid. Reading any fields from the ELF file is safe
/// **only after** calling [`Elf::parse`].
///
/// Further, debug-printing (via [`fmt::Debug`]) is safe **only**
/// after calling [`Elf::parse`].
///
/// To avoid any safety issues, do not cast pointers to this type,
/// despite it being a bitwise representation of the ELF file.
/// Parse returns a safe reference to the ELF file at the same
/// address without allocating.
#[repr(C)]
pub struct Elf {
	/// The ident section (shared between ELF32 and ELF64).
	ident:  ElfIdent,
	/// The ELF32/64 type, which is architecture-dependent.
	endian: ElfArch,
}

impl Elf {
	/// "Parses" (really, validates) an ELF file in memory.
	///
	/// `base_addr` must be aligned to a 4-byte boundary.
	///
	/// # Safety
	/// The ELF file must be loaded into memory and readable.
	/// The memory does not need to be executable.
	///
	/// The memory must outlive the returned reference
	/// (e.g. `'static` lifetime).
	///
	/// `length` is the length of the ELF file in bytes.
	/// It may be _longer_ than the actual ELF file, but
	/// it must not be shorter, and must always refer
	/// to otherwise valid memory not used by anything else.
	///
	/// `length` must not be larger than an `isize`.
	///
	/// The parser will check that all data falls within
	/// the length, but does not enforce that the ELF is
	/// _exactly_ `length` bytes long.
	pub unsafe fn parse(
		base_addr: *const u8,
		length: usize,
		endianness: ElfEndianness,
		class: ElfClass,
		machine: ElfMachine,
	) -> Result<&'static Self, ElfError> {
		#[expect(clippy::cast_ptr_alignment)]
		if !base_addr.cast::<Self>().is_aligned() {
			return Err(ElfError::UnalignedBaseAddr);
		}

		// SAFETY: `length` requirements offloaded to caller.
		let end_excl = unsafe { base_addr.add(length) as u64 };

		// SAFETY: We've checked that the base address is aligned.
		#[expect(clippy::cast_ptr_alignment)]
		let elf = unsafe { &*base_addr.cast::<Self>() };

		if elf.ident.magic != [0x7F, b'E', b'L', b'F'] {
			return Err(ElfError::InvalidMagic);
		}

		if !matches!(elf.ident.class as u8, 1_u8 | 2_u8) {
			return Err(ElfError::InvalidClass(elf.ident.class as u8));
		}

		if elf.ident.class != class {
			return Err(ElfError::ClassMismatch {
				elf:      elf.ident.class,
				expected: class,
			});
		}

		if !matches!(elf.ident.endian as u8, 1_u8 | 2_u8) {
			return Err(ElfError::InvalidEndianness(elf.ident.endian as u8));
		}

		if elf.ident.endian != endianness {
			return Err(ElfError::EndiannessMismatch {
				elf:      elf.ident.endian,
				expected: endianness,
			});
		}

		if elf.ident.version != 1 {
			return Err(ElfError::InvalidIdentVersion(elf.ident.version));
		}

		// XXX(qix-): We don't really care much about the OS ABI
		// XXX(qix-): given how Oro is designed. In reality, we
		// XXX(qix-): may not even need to check this field.
		// XXX(qix-): For now, just to be safe, we'll enforce
		// XXX(qix-): System V's ABI version 0. This restriction
		// XXX(qix-): may be relaxed in the future, especially
		// XXX(qix-): if Oro ever implements a custom ABI.
		if elf.ident.os_abi != 0 {
			return Err(ElfError::InvalidAbi(elf.ident.os_abi));
		}

		if elf.ident.abi_version != 0 {
			return Err(ElfError::InvalidAbiVersion(elf.ident.abi_version));
		}

		/// Validate arch-specific fields.
		macro_rules! validate_arch_header {
			($hdr:expr, $end_excl:expr) => {{
				// 2 == `ET_EXEC`, which is the only thing we support.
				if $hdr.ty != 2 {
					return Err(ElfError::NotExecutable($hdr.ty));
				}

				if $hdr.machine != machine {
					return Err(ElfError::MachineMismatch {
						elf:      $hdr.machine,
						expected: machine,
					});
				}

				if $hdr.version != 1 {
					return Err(ElfError::InvalidFileVersion($hdr.version));
				}

				if u64::from($hdr.ph_offset) >= $end_excl {
					return Err(ElfError::ProgHeaderOffsetOutOfBounds);
				}

				if u64::from($hdr.sh_offset) >= $end_excl {
					return Err(ElfError::SectHeaderOffsetOutOfBounds);
				}

				let ph_end = u64::from($hdr.ph_offset)
					+ (u64::from($hdr.ph_entry_size) * u64::from($hdr.ph_entry_count));

				if ph_end > $end_excl {
					return Err(ElfError::ProgHeaderTooLong);
				}

				let sh_end = u64::from($hdr.sh_offset)
					+ (u64::from($hdr.sh_entry_size) * u64::from($hdr.sh_entry_count));

				if sh_end > $end_excl {
					return Err(ElfError::SectHeaderTooLong);
				}

				// We don't use the string index, so we don't need to validate it.
			}};
		}

		// SAFETY: Access to union field is checked and safe in this case.
		unsafe {
			match elf.ident.class {
				ElfClass::Class32 => validate_arch_header!(&elf.endian.elf32, end_excl),
				ElfClass::Class64 => validate_arch_header!(&elf.endian.elf64, end_excl),
			}
		}

		Ok(elf)
	}

	/// Returns an iterator over the segments of the ELF file.
	///
	/// This iterator will skip segments that are not supported
	/// by Oro.
	#[must_use]
	pub fn segments(&self) -> SegmentIterator<'_> {
		SegmentIterator {
			elf:   self,
			index: 0,
		}
	}

	/// Returns the entry point of the ELF file.
	#[inline]
	#[must_use]
	#[expect(clippy::cast_possible_truncation)]
	pub fn entry_point(&self) -> usize {
		match self.ident.class {
			ElfClass::Class32 => unsafe { self.endian.elf32.entry as usize },
			ElfClass::Class64 => unsafe { self.endian.elf64.entry as usize },
		}
	}
}

/// The identity section of an ELF file. Common
/// between ELF32 and ELF64.
#[derive(Debug)]
#[repr(C, align(4))]
struct ElfIdent {
	/// The magic header of the ELF file (`EI_MAG0`..`EI_MAG3`).
	magic:       [u8; 4],
	/// The class of the ELF file (`EI_CLASS`).
	class:       ElfClass,
	/// Endianness of the ELF file (`EI_DATA`).
	endian:      ElfEndianness,
	/// ELF version (`EI_VERSION`). Must be 1.
	version:     u8,
	/// Target OS ABI (`EI_OSABI`).
	os_abi:      u8,
	/// ABI version (`EI_ABIVERSION`).
	abi_version: u8,
	/// Padding (`EI_PAD`..`EI_NIDENT-1`).
	_padding:    [u8; 7],
}

// `EI_NIDENT` is 16.
const _: () = assert::size_of::<ElfIdent, 16>();

/// An architecture-dependent ELF header (either [`Elf32`] or [`Elf64`]).
#[repr(C, align(4))]
union ElfArch {
	/// An ELF32 header.
	elf32: ManuallyDrop<Elf32>,
	/// An ELF64 header.
	elf64: ManuallyDrop<Elf64>,
}

impl fmt::Debug for Elf {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut s = f.debug_struct("Elf");

		s.field("ident", &self.ident);

		match self.ident.class {
			ElfClass::Class32 => {
				let elfhdr = unsafe { &*self.endian.elf32 };
				s.field("elf32", elfhdr);
			}
			ElfClass::Class64 => {
				let elfhdr = unsafe { &*self.endian.elf64 };
				s.field("elf64", elfhdr);
			}
		}

		s.field_with("segments", |f| {
			let mut segments = f.debug_list();

			match self.ident.class {
				ElfClass::Class32 => {
					let elfhdr = unsafe { &self.endian.elf32 };
					for i in 0..elfhdr.ph_entry_count {
						let offset = (from_ref(self) as u32)
							+ elfhdr.ph_offset + (u32::from(i)
							* u32::from(elfhdr.ph_entry_size));

						let segment = unsafe { &*(offset as *const ElfProgHeader32) };

						segments.entry(segment);
					}
				}
				ElfClass::Class64 => {
					let elfhdr = unsafe { &self.endian.elf64 };
					for i in 0..elfhdr.ph_entry_count {
						let offset = (from_ref(self) as u64)
							+ elfhdr.ph_offset + (u64::from(i)
							* u64::from(elfhdr.ph_entry_size));

						let segment = unsafe { &*(offset as *const ElfProgHeader64) };

						segments.entry(segment);
					}
				}
			}

			segments.finish()
		});

		s.finish()
	}
}

/// A segment iterator over an [`Elf`] file.
///
/// Note that segment types that are unsupported
/// are **skipped**.
pub struct SegmentIterator<'a> {
	/// Reference to the ELF file
	elf:   &'a Elf,
	/// The current segment index
	index: u16,
}

impl<'a> Iterator for SegmentIterator<'a> {
	type Item = ElfSegmentHeader<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		let total_entries = match self.elf.ident.class {
			ElfClass::Class32 => unsafe { self.elf.endian.elf32.ph_entry_count },
			ElfClass::Class64 => unsafe { self.elf.endian.elf64.ph_entry_count },
		};

		let start_index = self.index;

		for index in start_index..total_entries {
			self.index = index + 1;

			if index >= total_entries {
				return None;
			}

			let result = match self.elf.ident.class {
				ElfClass::Class32 => {
					let elfhdr = unsafe { &self.elf.endian.elf32 };
					let offset = (from_ref(self.elf) as u32)
						+ elfhdr.ph_offset + (u32::from(index)
						* u32::from(elfhdr.ph_entry_size));

					let segment = unsafe { &*(offset as *const ElfProgHeader32) };

					ElfSegmentHeader::Elf32(self.elf, segment)
				}
				ElfClass::Class64 => {
					let elfhdr = unsafe { &self.elf.endian.elf64 };
					let offset = (from_ref(self.elf) as u64)
						+ elfhdr.ph_offset + (u64::from(index)
						* u64::from(elfhdr.ph_entry_size));

					let segment = unsafe { &*(offset as *const ElfProgHeader64) };

					ElfSegmentHeader::Elf64(self.elf, segment)
				}
			};

			if result.ty() == ElfSegmentType::Ignored {
				continue;
			}

			return Some(result);
		}

		None
	}
}

/// Allows the [`SegmentIterator`] to switch between
/// the two segment types.
pub enum ElfSegmentHeader<'a> {
	/// 32-bit ELF segment.
	Elf32(&'a Elf, &'a ElfProgHeader32),
	/// 64-bit ELF segment.
	Elf64(&'a Elf, &'a ElfProgHeader64),
}

/// Provides unified access over ELF segments.
pub trait ElfSegment {
	/// The type of the ELF segment.
	fn ty(&self) -> ElfSegmentType;
	/// Load base virtual address
	fn load_address(&self) -> usize;
	/// Target base virtual address
	fn target_address(&self) -> usize;
	/// Size of the segment in memory
	/// (the number of bytes loaded)
	fn load_size(&self) -> usize;
	/// Size of the target segment in memory
	/// (the number of bytes the segment occupies;
	/// if larger than load size, the remaining
	/// bytes are zeroed)
	fn target_size(&self) -> usize;
}

impl fmt::Debug for ElfSegmentHeader<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ElfSegment")
			.field("ty", &self.ty())
			.field("load_address", &self.load_address())
			.field("target_address", &self.target_address())
			.field("load_size", &self.load_size())
			.field("target_size", &self.target_size())
			.finish()
	}
}

impl ElfSegment for ElfSegmentHeader<'_> {
	fn ty(&self) -> ElfSegmentType {
		let (flags, ptype) = match self {
			ElfSegmentHeader::Elf32(_, hdr) => (hdr.flags, hdr.ty),
			ElfSegmentHeader::Elf64(_, hdr) => (hdr.flags, hdr.ty),
		};

		if ptype != 1 {
			return ElfSegmentType::Ignored;
		}

		let is_x = flags & 1 != 0;
		let is_w = flags & 2 != 0;
		let is_r = flags & 4 != 0;
		let os_flags = flags & (0xFF << 20);

		if os_flags & ORO_ELF_FLAGTYPE_KERNEL_CODE != 0 {
			match (is_x, is_w, is_r) {
				(true, false, true) => ElfSegmentType::KernelCode,
				(false, true, true) => ElfSegmentType::KernelData,
				(false, false, true) => {
					if os_flags & ORO_ELF_FLAGTYPE_KERNEL_BOOT_PROTOCOL != 0 {
						ElfSegmentType::KernelRequests
					} else {
						ElfSegmentType::KernelRoData
					}
				}
				_ => ElfSegmentType::Invalid { flags, ptype },
			}
		} else {
			match (is_x, is_w, is_r) {
				(true, false, true) => ElfSegmentType::ModuleCode,
				(false, true, true) => ElfSegmentType::ModuleData,
				(false, false, true) => ElfSegmentType::ModuleRoData,
				_ => ElfSegmentType::Invalid { flags, ptype },
			}
		}
	}

	#[expect(clippy::cast_possible_truncation)]
	#[inline]
	fn load_address(&self) -> usize {
		match self {
			// `elf` in this match is a &&ELF. We have to de-ref it
			// to get the actual ELF.
			//
			// (qix-) Time wasted debugging: 4 hours.
			ElfSegmentHeader::Elf32(elf, hdr) => {
				// Make sure we get the correct ref address.
				let elf: &Elf = elf;
				from_ref(elf) as usize + hdr.offset as usize
			}
			ElfSegmentHeader::Elf64(elf, hdr) => {
				// Make sure we get the correct ref address.
				let elf: &Elf = elf;
				from_ref(elf) as usize + hdr.offset as usize
			}
		}
	}

	#[expect(clippy::cast_possible_truncation)]
	#[inline]
	fn target_address(&self) -> usize {
		match self {
			ElfSegmentHeader::Elf32(_, hdr) => hdr.virt as usize,
			ElfSegmentHeader::Elf64(_, hdr) => hdr.virt as usize,
		}
	}

	#[expect(clippy::cast_possible_truncation)]
	#[inline]
	fn load_size(&self) -> usize {
		match self {
			ElfSegmentHeader::Elf32(_, hdr) => hdr.file_size as usize,
			ElfSegmentHeader::Elf64(_, hdr) => hdr.file_size as usize,
		}
	}

	#[expect(clippy::cast_possible_truncation)]
	#[inline]
	fn target_size(&self) -> usize {
		match self {
			ElfSegmentHeader::Elf32(_, hdr) => hdr.mem_size as usize,
			ElfSegmentHeader::Elf64(_, hdr) => hdr.mem_size as usize,
		}
	}
}

/// The type of an ELF segment.
///
/// Note that these types **do not** map 1:1 to the
/// actual ELF segment types. The iterator takes
/// into account the OS-specific bits and returns
/// Oro-specific segment types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfSegmentType {
	/// Ignored
	Ignored,
	/// Invalid segment. This is returned when
	/// the segment uses OS flag bits but they're
	/// either not supported or have the wrong
	/// combination of permissions bits.
	Invalid {
		/// The flags of the segment
		flags: u32,
		/// The type of the segment
		ptype: u32,
	},
	/// Kernel code segment
	KernelCode,
	/// Kernel data segment (read-write)
	KernelData,
	/// Kernel read-only data segment
	KernelRoData,
	/// Kernel configuration requests (read-only)
	KernelRequests,
	/// Module code segment
	ModuleCode,
	/// Module data segment (read-write)
	ModuleData,
	/// Module read-only data segment
	ModuleRoData,
}

impl ElfSegmentType {
	/// Returns `true` if the segment is a kernel segment.
	#[must_use]
	pub fn is_kernel_segment(&self) -> bool {
		matches!(
			self,
			Self::KernelCode | Self::KernelData | Self::KernelRoData | Self::KernelRequests
		)
	}
}

/// An ELF64 header.
///
/// Do not use this directly; use [`Elf`] instead.
#[derive(Debug)]
#[repr(C, align(4))]
struct Elf64 {
	/// The type of the ELF file.
	pub ty: u16,
	/// The machine architecture.
	pub machine: ElfMachine,
	/// The version of the ELF file.
	pub version: u32,
	/// The entry point of the ELF file.
	pub entry: u64,
	/// The program header table offset.
	pub ph_offset: u64,
	/// The section header table offset.
	pub sh_offset: u64,
	/// Flags.
	pub flags: u32,
	/// The size of this header.
	pub header_size: u16,
	/// The size of a program header entry.
	pub ph_entry_size: u16,
	/// The number of program header entries.
	pub ph_entry_count: u16,
	/// The size of a section header entry.
	pub sh_entry_size: u16,
	/// The number of section header entries.
	pub sh_entry_count: u16,
	/// The index of the section header table entry that contains the section names.
	pub sh_str_index: u16,
}

/// An ELF32 header.
///
/// Do not use this directly; use [`Elf`] instead.
#[derive(Debug)]
#[repr(C, align(4))]
struct Elf32 {
	/// The type of the ELF file.
	pub ty: u16,
	/// The machine architecture.
	pub machine: ElfMachine,
	/// The version of the ELF file.
	pub version: u32,
	/// The entry point of the ELF file.
	pub entry: u32,
	/// The program header table offset.
	pub ph_offset: u32,
	/// The section header table offset.
	pub sh_offset: u32,
	/// Flags.
	pub flags: u32,
	/// The size of this header.
	pub header_size: u16,
	/// The size of a program header entry.
	pub ph_entry_size: u16,
	/// The number of program header entries.
	pub ph_entry_count: u16,
	/// The size of a section header entry.
	pub sh_entry_size: u16,
	/// The number of section header entries.
	pub sh_entry_count: u16,
	/// The index of the section header table entry that contains the section names.
	pub sh_str_index: u16,
}

/// The valid values of ELF class (`EI_CLASS`).
///
/// # Safety
/// It is ONLY safe to read this valid from [`Elf`]
/// references if they have had [`Elf::parse`] called
/// on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ElfClass {
	/// 32-bit ELF
	Class32 = 1,
	/// 64-bit ELF
	Class64 = 2,
}

/// The valid values of ELF endianness (`EI_DATA`).
///
/// # Safety
/// It is ONLY safe to read this valid from [`Elf`]
/// references if they have had [`Elf::parse`] called
/// on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ElfEndianness {
	/// Little-endian
	Little = 1,
	/// Big-endian
	Big    = 2,
}

/// The valid values of the ELF machine (`e_machine`).
///
/// # Safety
/// It is ONLY safe to read this valid from [`Elf`]
/// references if they have had [`Elf::parse`] called
/// on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[non_exhaustive]
pub enum ElfMachine {
	/// AMD x86-64
	X86_64  = 0x3E,
	/// ARM Aarch64
	Aarch64 = 0xB7,
}

/// A program header for 32-bit ELF files.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(4))]
pub struct ElfProgHeader32 {
	/// The type of the program header.
	ty:        u32,
	/// The offset of the segment in the file.
	offset:    u32,
	/// The virtual address of the segment in memory.
	virt:      u32,
	/// The physical address of the segment in memory.
	phys:      u32,
	/// The size of the segment in the file.
	file_size: u32,
	/// The size of the segment in memory.
	mem_size:  u32,
	/// Flags.
	flags:     u32,
	/// The alignment of the segment.
	align:     u32,
}

/// A program header for 64-bit ELF files.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(4))]
pub struct ElfProgHeader64 {
	/// The type of the program header.
	ty:        u32,
	/// Flags.
	flags:     u32,
	/// The offset of the segment in the file.
	offset:    u64,
	/// The virtual address of the segment in memory.
	virt:      u64,
	/// The physical address of the segment in memory.
	phys:      u64,
	/// The size of the segment in the file.
	file_size: u64,
	/// The size of the segment in memory.
	mem_size:  u64,
	/// The alignment of the segment.
	align:     u64,
}

/// Errors that can occur when parsing/validating an ELF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
	/// The base address is not aligned to a 4-byte boundary.
	UnalignedBaseAddr,
	/// The magic number is invalid.
	InvalidMagic,
	/// Invalid class
	InvalidClass(u8),
	/// Invalid endianness
	InvalidEndianness(u8),
	/// Class mismatch between ELF file and architecture
	ClassMismatch {
		/// The ELF class
		elf:      ElfClass,
		/// The expected ELF class
		expected: ElfClass,
	},
	/// Endianness mismatch between ELF file and architecture
	EndiannessMismatch {
		/// The ELF endianness
		elf:      ElfEndianness,
		/// The expected ELF endianness
		expected: ElfEndianness,
	},
	/// Invalid ELF ident section version
	InvalidIdentVersion(u8),
	/// Invalid ELF file version
	InvalidFileVersion(u32),
	/// Invalid ABI
	InvalidAbi(u8),
	/// Invalid ABI version
	InvalidAbiVersion(u8),
	/// Invalid machine
	InvalidMachine(u16),
	/// Machine mismatch between ELF file and architecture
	MachineMismatch {
		/// The ELF machine
		elf:      ElfMachine,
		/// The expected ELF machine
		expected: ElfMachine,
	},
	/// The ELF file is not executable.
	NotExecutable(u16),
	/// The program header offset is out of bounds for the ELF file.
	ProgHeaderOffsetOutOfBounds,
	/// The section header offset is out of bounds for the ELF file.
	SectHeaderOffsetOutOfBounds,
	/// The program header entries extend beyond the end of the ELF file.
	ProgHeaderTooLong,
	/// The section header entries extend beyond the end of the ELF file.
	SectHeaderTooLong,
}
