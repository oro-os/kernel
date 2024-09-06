//! DeviceTree blob reader support for the Oro kernel.
#![cfg_attr(not(test), no_std)]

use oro_type::Be;

/// The flattened DeviceTree blob header.
///
/// Documented in section 5.2 of the DeviceTree specification
/// <https://www.devicetree.org/specifications/>.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct FdtHeader {
	/// This field shall contain the value `0xd00dfeed` (big-endian).
	magic: Be<u32>,
	/// This field shall contain the total size in bytes of the devicetree
	/// data structure.
	///
	/// This size shall encompass all sections of the structure: the header,
	/// the memory reservation block, structure block and strings block, as
	/// well as any free space gaps between the blocks or after the final block.
	totalsize: Be<u32>,
	/// This field shall contain the offset in bytes of the structure block
	/// (see section 5.4) from the beginning of the header.
	off_dt_struct: Be<u32>,
	/// This field shall contain the offset in bytes of the strings block
	/// (see Section 5.5) from the beginning of the header
	off_dt_strings: Be<u32>,
	/// This field shall contain the offset in bytes of the memory reservation
	/// block (see Section 5.3) from the beginning of the header.
	off_mem_rsvmap: Be<u32>,
	/// This field shall contain the version of the devicetree data structure.
	///
	/// The version is 17 if using the structure as defined in this document.
	/// An DTSpec boot program may provide the devicetree of a later version,
	/// in which case this field shall contain the version number defined in
	/// whichever later document gives the details of that version.
	version: Be<u32>,
	/// This field shall contain the lowest version of the devicetree data structure
	/// with which the version used is backwards compatible.
	///
	/// So, for the structure as
	/// defined in this document (version 17), this field shall contain 16 because
	/// version 17 is backwards compatible with version 16, but not earlier versions.
	/// As per Section 5.1, a DTSpec boot program should provide a devicetree in a
	/// format which is backwards compatible with version 16, and thus this field
	/// shall always contain 16.
	last_comp_version: Be<u32>,
	/// This field shall contain the physical ID of the system’s boot CPU.
	///
	/// It shall be identical to the physical ID given in the reg property of that CPU node within the devicetree.
	boot_cpuid_phys: Be<u32>,
	/// This field shall contain the length in bytes of the strings block section of the devicetree blob.
	size_dt_strings: Be<u32>,
	/// This field shall contain the length in bytes of the structure block section of the devicetree blob.
	size_dt_struct: Be<u32>,
}

impl FdtHeader {
	/// Validates the header.
	///
	/// If `len` is provided, the header is validated against the given length.
	pub fn validate(&self, len: Option<u32>) -> Result<(), ValidationError> {
		if self.magic.read() != 0xD00D_FEED {
			return Err(ValidationError::BadMagic);
		}

		if let Some(len) = len {
			if self.totalsize.read() != len {
				return Err(ValidationError::LengthMismatch {
					expected: len,
					reported: self.totalsize.read(),
				});
			}
		}

		if self.version.read() != 17 && self.last_comp_version.read() > 17 {
			return Err(ValidationError::VersionMismatch {
				expected:   17,
				reported:   self.version.read(),
				compatible: self.last_comp_version.read(),
			});
		}

		Ok(())
	}
}

/// An error that occurs when validating a DeviceTree structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
	/// Magic number mismatch (expectes `0xd00dfeed`).
	BadMagic,
	/// Length mismatch
	LengthMismatch {
		/// Expected length
		expected: u32,
		/// Reported length (by the DeviceTree header)
		reported: u32,
	},
	/// Version mismatch
	///
	/// **Note:** The least compatible version is checked, not
	/// just the actual version number. If this implementation
	/// is _compatible_ with the version of DTB that was provided,
	/// validation will still pass.
	VersionMismatch {
		/// Expected version
		expected:   u32,
		/// Reported version
		reported:   u32,
		/// Lowest compatible version
		compatible: u32,
	},
}
