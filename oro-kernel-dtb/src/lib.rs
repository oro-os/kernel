//! DeviceTree blob reader support for the Oro kernel.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(doc, feature(doc_cfg, doc_auto_cfg))]

use core::{ffi::CStr, mem::size_of, ptr::from_ref};

use oro_kernel_type::Be;

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
	/// Validates the header and constructs a static reference to the flattened DTB header.
	///
	/// If `len` is provided, the header is validated against the given length.
	pub fn from(ptr: *const u8, len: Option<u32>) -> Result<&'static Self, ValidationError> {
		#[expect(clippy::cast_ptr_alignment)]
		let ptr = ptr.cast::<Self>();

		if !ptr.is_aligned() {
			return Err(ValidationError::Unaligned);
		}

		let magic = unsafe { ptr.cast::<Be<u32>>().read() }.read();

		if magic != 0xD00D_FEED {
			return Err(ValidationError::BadMagic);
		}

		let this = unsafe { &*ptr };

		if let Some(len) = len {
			if this.totalsize.read() != len {
				return Err(ValidationError::LengthMismatch {
					expected: len,
					reported: this.totalsize.read(),
				});
			}
		}

		if this.version.read() != 17 && this.last_comp_version.read() > 17 {
			return Err(ValidationError::VersionMismatch {
				expected:   17,
				reported:   this.version.read(),
				compatible: this.last_comp_version.read(),
			});
		}

		if this.off_dt_struct.read() % 8 != 0 {
			return Err(ValidationError::StructUnaligned);
		}

		Ok(this)
	}

	/// Returns the bootstrap (primary) processor's physical ID.
	#[must_use]
	pub fn phys_id(&self) -> u32 {
		self.boot_cpuid_phys.read()
	}

	/// Returns the byte slice of the structure block.
	#[expect(clippy::needless_lifetimes)]
	fn struct_slice<'a>(&'a self) -> &'a [u8] {
		unsafe {
			core::slice::from_raw_parts(
				from_ref(self)
					.cast::<u8>()
					.add(self.off_dt_struct.read() as usize),
				self.size_dt_struct.read() as usize,
			)
		}
	}

	/// Returns the byte slice of the string block.
	#[expect(clippy::needless_lifetimes)]
	fn string_slice<'a>(&'a self) -> &'a [u8] {
		unsafe {
			core::slice::from_raw_parts(
				from_ref(self)
					.cast::<u8>()
					.add(self.off_dt_strings.read() as usize),
				self.size_dt_strings.read() as usize,
			)
		}
	}

	/// Returns an iterator over the raw DTB tokens.
	#[expect(clippy::needless_lifetimes)]
	pub fn iter<'a>(&'a self) -> impl Iterator<Item = FdtToken<'a>> + 'a {
		FdtIter::new(self).fuse()
	}
}

/// An error that occurs when validating a DeviceTree structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
	/// The pointer to the DTB header is unaligned (must be aligned
	/// to an 8-byte boundary).
	Unaligned,
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
	/// The structure offset is not 8-byte aligned.
	StructUnaligned,
}

/// A single token in a DeviceTree blob.
#[derive(Debug, Clone, Copy)]
pub enum FdtToken<'a> {
	/// A property token.
	Property {
		/// The property's name.
		name:  &'a CStr,
		/// The property's value.
		value: &'a [u8],
	},
	/// A node token.
	Node {
		/// The node's name.
		name: &'a CStr,
	},
	/// An end node token.
	EndNode,
	/// A NOP token.
	Nop,
	/// An end token.
	End,
}

/// Iterates a DTB structure and returns tokens.
///
/// Returned by [`FdtHeader::iter`].
pub struct FdtIter<'a> {
	/// The current offset in the DTB structure
	/// section.
	offset:       u32,
	/// The slice of structure data.
	struct_slice: &'a [u8],
	/// The slice of string data.
	string_slice: &'a [u8],
}

impl<'a> FdtIter<'a> {
	/// Creates a new iterator for the given DTB structure.
	fn new(dtb: &'a FdtHeader) -> Self {
		Self {
			offset:       0,
			struct_slice: dtb.struct_slice(),
			string_slice: dtb.string_slice(),
		}
	}
}

impl<'a> FdtIter<'a> {
	/// Yields the next `n` bytes from the structure slice.
	///
	/// Returns `None` if the slice is exhausted, or if there
	/// are fewer than `n` bytes remaining.
	fn next_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
		if self.offset as usize + n > self.struct_slice.len() {
			None
		} else {
			let slice = &self.struct_slice[self.offset as usize..self.offset as usize + n];
			self.offset += n as u32;
			Some(slice)
		}
	}

	/// Yields the next `T` from the structure slice.
	fn next_item<T: Copy + Sized>(&mut self) -> Option<T> {
		self.next_bytes(size_of::<T>()).map(|bytes| {
			let ptr = bytes.as_ptr().cast::<T>();
			unsafe { ptr.read() }
		})
	}
}

impl<'a> Iterator for FdtIter<'a> {
	type Item = FdtToken<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		match self.next_item::<Be<u32>>()?.read() {
			0x0000_0009 => Some(FdtToken::End),
			0x0000_0004 => Some(FdtToken::Nop),
			0x0000_0002 => Some(FdtToken::EndNode),
			0x0000_0003 => {
				let len = self.next_item::<Be<u32>>()?.read() as usize;
				let nameoff = self.next_item::<Be<u32>>()?.read() as usize;

				let value = self.next_bytes(len)?;
				// Pad to the next 32-bit boundary.
				self.offset = (self.offset + 3) & !3;

				// If this errors, it means there's no nul bytes in the string slice,
				// indicating a malformed DTB. We just return a None in this case and
				// let the caller handle it.
				let name = CStr::from_bytes_until_nul(&self.string_slice[nameoff..]).ok()?;

				Some(FdtToken::Property { name, value })
			}
			0x0000_0001 => {
				let name =
					CStr::from_bytes_until_nul(&self.struct_slice[self.offset as usize..]).ok()?;
				self.offset += (
					name.count_bytes() as u32 + 4
					// 3 + NUL
				) & !3;

				Some(FdtToken::Node { name })
			}
			_ => None,
		}
	}
}

/// Filters iterators over [`FdtToken`]s by path.
pub trait FdtPathFilter<'a>: Iterator<Item = FdtToken<'a>> + Sized {
	/// Creates a filter iterator that yields only the tokens
	/// that match the given path.
	///
	/// # Important
	/// Should only be constructed on an iterator that has not
	/// yet yielded any tokens.
	fn filter_path(self, path: &'a [&'a CStr]) -> FdtFilterIter<'a, Self> {
		FdtFilterIter {
			iter: self,
			path,
			depth: 0,
			last_match_depth: 0,
		}
	}
}

impl<'a, I: Iterator<Item = FdtToken<'a>> + Sized> FdtPathFilter<'a> for I {}

/// A path filter for a DeviceTree iterator.
pub struct FdtFilterIter<'a, I: Iterator<Item = FdtToken<'a>>> {
	/// The underlying iterator.
	iter: I,
	/// The canonical path to filter by.
	///
	/// Elements are leafs in the path, e.g.
	/// `&["memory", "reg"]` corresponds to
	/// `/memory/reg`.
	///
	/// At each of the leaf segments, the presence
	/// or absence of an `@id` suffix behaves as follows:
	///
	/// - The absence of `@` in the leaf segment
	///   matches _only_ nodes without an ID.
	/// - Leafs ending with `@` match any node with
	///   an ID, regardless of the ID. Nodes that
	///   do not have an ID are not matched, even if
	///   the base leaf name matches.
	/// - Leafs ending with `@id` match only nodes
	///   with the given ID.
	path: &'a [&'a CStr],
	/// The current node depth.
	depth: usize,
	/// The depth of the last matched path segment.
	last_match_depth: usize,
}

impl<'a, I: Iterator<Item = FdtToken<'a>>> Iterator for FdtFilterIter<'a, I> {
	type Item = FdtToken<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			match self.iter.next()? {
				FdtToken::End => return None,
				FdtToken::Nop => {}
				prop @ FdtToken::Property { .. } => {
					if self.last_match_depth == self.path.len() {
						return Some(prop);
					}
				}
				node @ FdtToken::Node { name } => {
					let should_check = self.depth == self.last_match_depth
						&& self.last_match_depth < self.path.len();

					if should_check {
						let leaf = &self.path[self.depth];
						let checks_id = leaf.to_bytes().contains(&b'@');
						let checks_exact =
							!checks_id || leaf.to_bytes().last().is_none_or(|&c| c != b'@');

						let matches = if checks_exact {
							leaf == &name
						} else {
							name.to_bytes().starts_with(leaf.to_bytes())
						};

						if matches {
							self.last_match_depth += 1;
						}
					}

					self.depth += 1;

					if self.last_match_depth == self.path.len() {
						return Some(node);
					}
				}
				end_node @ FdtToken::EndNode => {
					let should_return = self.last_match_depth == self.path.len();

					if self.depth == self.last_match_depth {
						self.last_match_depth -= 1;
					}

					self.depth -= 1;

					if should_return {
						return Some(end_node);
					}
				}
			}
		}
	}
}
