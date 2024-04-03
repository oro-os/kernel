//! ELF parser. This parser is quite bare-bones as it's only
//! needed for loading the kernel and modules and extracting
//! the module metadata.

use crate::Arch;
use core::{
	fmt,
	mem::{transmute, ManuallyDrop},
	ptr::from_ref,
};

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
	/// The parser will check that all data falls within
	/// the length, but does not enforce that the ELF is
	/// _exactly_ `length` bytes long.
	pub unsafe fn parse<A: Arch>(base_addr: usize, length: u64) -> Result<&'static Self, ElfError> {
		if base_addr & 3 != 0 {
			return Err(ElfError::UnalignedBaseAddr);
		}

		let end_excl: u64 = base_addr as u64 + length;

		let elf = &*(base_addr as *const Self);

		if elf.ident.magic != [0x7F, b'E', b'L', b'F'] {
			return Err(ElfError::InvalidMagic);
		}

		if !matches!(transmute(elf.ident.class), 1_u8 | 2_u8) {
			return Err(ElfError::InvalidClass(transmute(elf.ident.class)));
		}

		if elf.ident.class != A::ELF_CLASS {
			return Err(ElfError::ClassMismatch {
				elf:  elf.ident.class,
				arch: A::ELF_CLASS,
			});
		}

		if !matches!(transmute(elf.ident.endian), 1_u8 | 2_u8) {
			return Err(ElfError::InvalidEndianness(transmute(elf.ident.endian)));
		}

		if elf.ident.endian != A::ELF_ENDIANNESS {
			return Err(ElfError::EndiannessMismatch {
				elf:  elf.ident.endian,
				arch: A::ELF_ENDIANNESS,
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
			($Arch:ty, $hdr:expr, $end_excl:expr) => {{
				// 2 == `ET_EXEC`, which is the only thing we support.
				if $hdr.ty != 2 {
					return Err(ElfError::NotExecutable($hdr.ty));
				}

				if $hdr.machine != <$Arch>::ELF_MACHINE {
					return Err(ElfError::MachineMismatch {
						elf:  $hdr.machine,
						arch: <$Arch>::ELF_MACHINE,
					});
				}

				if $hdr.version != 1 {
					return Err(ElfError::InvalidVersion($hdr.version as u8));
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

		match elf.ident.class {
			ElfClass::Class32 => validate_arch_header!(A, &elf.endian.elf32, end_excl),
			ElfClass::Class64 => validate_arch_header!(A, &elf.endian.elf64, end_excl),
		}

		Ok(elf)
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
static_assertions::assert_eq_size!(ElfIdent, [u8; 16]);

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

		#[cfg(feature = "unstable")]
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
		elf:  ElfClass,
		/// The architecture's ELF class
		arch: ElfClass,
	},
	/// Endianness mismatch between ELF file and architecture
	EndiannessMismatch {
		/// The ELF endianness
		elf:  ElfEndianness,
		/// The architecture's ELF endianness
		arch: ElfEndianness,
	},
	/// Invalid ELF ident section version
	InvalidIdentVersion(u8),
	/// Invalid ELF version
	InvalidVersion(u8),
	/// Invalid ABI
	InvalidAbi(u8),
	/// Invalid ABI version
	InvalidAbiVersion(u8),
	/// Invalid machine
	InvalidMachine(u16),
	/// Machine mismatch between ELF file and architecture
	MachineMismatch {
		/// The ELF machine
		elf:  ElfMachine,
		/// The architecture's ELF machine
		arch: ElfMachine,
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
