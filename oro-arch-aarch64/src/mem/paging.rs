//! Some notes on the implementation of the aarch64 memory model
//! as it relates to the Oro kernel:
//!
//! - A granule size of 4KiB is assumed. This affects
//!   the page table sizes, which is currently set to 512 entries.
//!   In the future, this may be configurable, but will require
//!   different implementations for the page tables and an abstraction
//!   over them (e.g. the 16KiB granule size bifurcates the address
//!   space using bit 47 instead of using an L0 index).
//!
//! - All structures and manipulators are based on the
//!   assumption we are executing in a non-secure† state (EL1); thus
//!   many of the attribute bits have no manipulator methods.
//!
//! - The kernel currently does not provide hypervisor support, thus
//!   no considerations were made about how these structures may be
//!   used in stage 2 translations.
//!
//! - The terms "block" and "page" are used interchangeably in the
//!   implementation, but typically they're referred to as "block"
//!   descriptors so as to make physical page types and operations
//!   a bit more pronounced.
//!
//! For future reference, check D5.2.3 of the ARMv8-A Architecture
//! Reference Manual (ARM DDI 0487A.a) for more information.
//!
//! <sub>†: "Non-secure" is a specific term defined by the `ARMv8`
//! specification; it is not a general comment about the overall
//! security of the Oro kernel.</sub>
//!
//! Note that the address and type bits encoding for a [`L3PageTableBlock`]
//! _is the same†_ as [`L0PageTableDescriptor`], [`L1PageTableDescriptor`] and
//! [`L2PageTableDescriptor`], but the semantics of the address bits is different.
//!
//! <sub>†: See [Controlling address translation - Translation table format](https://developer.arm.com/documentation/101811/0103/Controlling-address-translation-Translation-table-format)
//! under final _Note_ block.</sub>

#![allow(clippy::inline_always, private_bounds)]

use core::{
	fmt,
	ops::{Index, IndexMut},
};
use oro_common::unsafe_precondition;

/// A single page table entry.
#[derive(Debug, Clone)]
#[repr(C, align(4096))]
pub struct PageTable {
	entries: [PageTableEntry; 512],
}

static_assertions::const_assert_eq!(::core::mem::size_of::<PageTable>(), 4096);

impl IndexMut<usize> for PageTable {
	#[inline(always)]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		debug_assert!(index < 512, "index out of bounds (max 511)");
		&mut self.entries[index]
	}
}

impl Index<usize> for PageTable {
	type Output = PageTableEntry;

	#[inline(always)]
	fn index(&self, index: usize) -> &Self::Output {
		debug_assert!(index < 512, "index out of bounds (max 511)");
		&self.entries[index]
	}
}

impl PageTable {
	/// Clears all entries in the page table.
	#[inline(always)]
	pub fn reset(&mut self) {
		for entry in &mut self.entries {
			entry.reset();
		}
	}
}

/// Describes the type of a page table entry, based on its level.
#[derive(Debug)]
pub enum PageTableEntryType<'a> {
	/// An invalid page table entry (bit 0 is not set).
	/// Note that this does _not_ mean the page table entry
	/// is malformed; it simply means that it is not "valid"
	/// as per the ARMv8 specification, meaning that the translation
	/// table walk will stop at this entry, failing the translation.
	Invalid(&'a mut PageTableEntry),
	/// A malformed page table entry. Returned
	/// when a level of 0 or 3 is given to [`PageTableEntry::entry_type`]
	/// but bit 1 is not set.
	///
	/// This is a special, erroneous case, that should
	/// be handled as an error if a well-formed page table entry
	/// was otherwise expected (i.e. not reading from uninitialized memory).
	///
	/// Functionally, the translation table walk will behave as though
	/// it were an [`PageTableEntryType::Invalid`] entry.
	Malformed(&'a mut PageTableEntry),
	/// An L0 page table descriptor entry.
	L0Descriptor(&'a mut L0PageTableDescriptor),
	/// An L1 page table descriptor entry.
	L1Descriptor(&'a mut L1PageTableDescriptor),
	/// An L2 page table descriptor entry.
	L2Descriptor(&'a mut L2PageTableDescriptor),
	/// An L1 page table block entry.
	L1Block(&'a mut L1PageTableBlockDescriptor),
	/// An L2 page table block entry.
	L2Block(&'a mut L2PageTableBlockDescriptor),
	/// An L3 page table block entry.
	L3Block(&'a mut L3PageTableBlockDescriptor),
}

/// A single page table entry.
#[derive(Clone, Copy)]
#[repr(C, align(8))]
pub struct PageTableEntry(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<PageTableEntry>(), 8);

impl PageTableEntry {
	/// Creates a new page table entry.
	#[inline(always)]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Resets the page table entry to its default state.
	#[inline(always)]
	pub fn reset(&mut self) {
		self.0 = 0;
	}

	/// Checks if the page table entry is a table descriptor.
	/// If `false`, it is a block descriptor.
	#[inline(always)]
	#[must_use]
	pub fn table(&self) -> bool {
		self.get_raw() & 0b1 << 1 != 0
	}

	/// Sets the page table entry as a table descriptor.
	#[inline(always)]
	pub fn set_table(&mut self) {
		*self.get_raw_mut() |= 0b1 << 1;
	}

	/// Clears the page table entry as a block descriptor.
	///
	/// # Safety
	/// Caller must ensure this is not being called on an
	/// otherwise well-formed L0 or L3 table descriptor,
	/// as this will result in a malformed entry.
	#[inline(always)]
	pub unsafe fn clear_table(&mut self) {
		*self.get_raw_mut() &= !(0b1 << 1);
	}

	/// Replaces the page table entry as a table descriptor.
	#[inline(always)]
	#[must_use]
	#[cfg(feature = "unstable")]
	pub const fn with_table(self) -> Self {
		Self(self.0 | 0b1 << 1)
	}

	/// Gets the address of the page table entry.
	/// If the page table is invalid or malformed, returns `None`.
	///
	/// **This is mostly for debugging purposes and should not be used
	/// in production code.**
	///
	/// # Safety
	/// Caller must ensure that `level` is `0..=3` and that it
	/// is correctly specified. **Do not assume this value.**
	#[must_use]
	pub unsafe fn address(&mut self, level: u8) -> Option<u64> {
		unsafe_precondition!(crate::Aarch64, level <= 3, "level must be 0..=3");

		match self.entry_type(level) {
			PageTableEntryType::Invalid(_) | PageTableEntryType::Malformed(_) => None,
			PageTableEntryType::L0Descriptor(desc) => Some(desc.address()),
			PageTableEntryType::L1Descriptor(desc) => Some(desc.address()),
			PageTableEntryType::L2Descriptor(desc) => Some(desc.address()),
			PageTableEntryType::L1Block(desc) => Some(desc.address()),
			PageTableEntryType::L2Block(desc) => Some(desc.address()),
			PageTableEntryType::L3Block(desc) => Some(desc.address()),
		}
	}

	/// Returns the type of the page table entry based
	/// on the level of the page table.
	///
	/// # Safety
	/// Caller must ensure that `level` is `0..=3` and that it
	/// is correctly specified. **Do not assume this value.**
	#[inline]
	#[must_use]
	pub unsafe fn entry_type(&mut self, level: u8) -> PageTableEntryType {
		unsafe_precondition!(crate::Aarch64, level <= 3, "level must be 0..=3");

		if !self.valid() {
			return PageTableEntryType::Invalid(self);
		}

		match level {
			0 => {
				if self.table() {
					PageTableEntryType::L0Descriptor(
						&mut *core::ptr::from_mut(self).cast::<L0PageTableDescriptor>(),
					)
				} else {
					PageTableEntryType::Malformed(self)
				}
			}
			1 => {
				if self.table() {
					PageTableEntryType::L1Descriptor(
						&mut *core::ptr::from_mut(self).cast::<L1PageTableDescriptor>(),
					)
				} else {
					PageTableEntryType::L1Block(
						&mut *core::ptr::from_mut(self).cast::<L1PageTableBlockDescriptor>(),
					)
				}
			}
			2 => {
				if self.table() {
					PageTableEntryType::L2Descriptor(
						&mut *core::ptr::from_mut(self).cast::<L2PageTableDescriptor>(),
					)
				} else {
					PageTableEntryType::L2Block(
						&mut *core::ptr::from_mut(self).cast::<L2PageTableBlockDescriptor>(),
					)
				}
			}
			3 => {
				// NOTE(qix-): This might look incorrect, but it's not.
				// NOTE(qix-): The "table" bit is set for L3 block entries.
				// NOTE(qix-): Bits [1:0] == 0b01 for L3 block entries is considered
				// NOTE(qix-): a "malformed" (reserved) bit representation and is treated
				// NOTE(qix-): as an invalid entry by the translation table walk.
				if self.table() {
					PageTableEntryType::L3Block(
						&mut *core::ptr::from_mut(self).cast::<L3PageTableBlockDescriptor>(),
					)
				} else {
					PageTableEntryType::Malformed(self)
				}
			}
			_ => unreachable!(),
		}
	}
}

impl fmt::Debug for PageTableEntry {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("PageTableEntry")
			.field("raw", &format_args!("{:016X?}", &self.0))
			.field("valid", &self.valid())
			.field("table", &self.table())
			.finish_non_exhaustive()
	}
}

/// A single page table entry subtype.
///
/// # Safety
/// The subtypes must be used properly and cannot be
/// cast between one another safely without determining
/// their type in accordance to their level (see [`PageTableEntry::entry_type`]).
pub trait PageTableEntrySubtype {
	/// The high bit of the address mask, inclusive.
	const ADDR_MASK_HIGH_BIT: u64;
	/// The low bit of the address mask, inclusive.
	const ADDR_MASK_LOW_BIT: u64;
	/// The computed bitmask for the address bits.
	const ADDR_MASK: u64 =
		((1 << (Self::ADDR_MASK_HIGH_BIT + 1)) - 1) & !((1 << (Self::ADDR_MASK_LOW_BIT)) - 1);
}

macro_rules! descriptor_init_value {
	(table) => {
		!0b1 | 0b10 | PageTableEntryTableAccessPerm::default_const() as u64
	};
	(block) => {
		!0b1 | PageTableEntryShareability::default_const() as u64
			| PageTableEntryBlockAccessPerm::default_const() as u64
	};
}

// FIXME(qix-): Workaround for a rustfmt bug where, when inlined
// FIXME(qix-): with the #[doc = ...] attribute on the subtype's
// FIXME(qix-): ::new() function, the doc comment keeps getting
// FIXME(qix-): indented whenever rustfmt runs.
macro_rules! descriptor_doc {
	($doc:literal) => {
		concat!(
			"Creates a new ",
			$doc,
			" with an initialization value.\n\n",
			"This constructor marks the descriptor as invalid, ",
			"but might set other default bits as necessary.\n\n",
			"# Safety\n",
			"Caller must ensure that the descriptor subtype is being used ",
			"at the correct level in the correct manner."
		)
	};
}

macro_rules! impl_descriptor_debug {
	(table $name:ident) => {
		impl fmt::Debug for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				f.debug_struct(stringify!($name))
					.field("raw", &format_args!("{:016X}", &self.0))
					.field("addr", &format_args!("{:016X}", &self.address()))
					.field("valid", &self.valid())
					.field("pxe", &self.kernel_no_exec())
					.field("uxe", &self.user_no_exec())
					.field("ap", &self.table_access_permissions())
					.finish()
			}
		}
	};

	(block $name:ident) => {
		impl fmt::Debug for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				f.debug_struct(stringify!($name))
					.field("raw", &format_args!("{:016X}", &self.0))
					.field("addr", &format_args!("{:016X}", &self.address()))
					.field("valid", &self.valid())
					.field("pxe", &self.kernel_no_exec())
					.field("uxe", &self.user_no_exec())
					.field("ap", &self.block_access_permissions())
					.field("cont", &self.contiguous())
					.field("ng", &self.not_global())
					.field("ns", &self.not_secure())
					.field("acc", &self.accessed())
					.field("mair", &self.mair_index())
					.finish()
			}
		}
	};
}

macro_rules! define_descriptor {
	($implty:tt $name:ident, $addr_mask_high:expr, $addr_mask_low:expr, $doc:literal) => {
		#[doc = concat!("An ", $doc, ".")]
		#[derive(Clone, Copy)]
		#[repr(C, align(8))]
		pub struct $name(u64);

		static_assertions::const_assert_eq!(::core::mem::size_of::<$name>(), 8);

		impl PageTableEntrySubtype for $name {
			const ADDR_MASK_HIGH_BIT: u64 = $addr_mask_high;
			const ADDR_MASK_LOW_BIT: u64 = $addr_mask_low;
		}

		impl $name {
			#[doc = descriptor_doc!($doc)]
			#[inline(always)]
			#[must_use]
			pub const fn new() -> Self {
				Self(descriptor_init_value!($implty))
			}

			/// Resets the page table entry to its initial state.
			#[inline(always)]
			pub fn reset(&mut self) {
				self.0 = descriptor_init_value!($implty);
			}
		}

		impl GetRaw for $name {
			#[inline(always)]
			fn get_raw(&self) -> u64 {
				self.0
			}

			#[inline(always)]
			fn get_raw_mut(&mut self) -> &mut u64 {
				&mut self.0
			}
		}

		#[cfg(feature = "unstable")]
		impl GetRawConst for $name {
			#[inline(always)]
			fn with(value: u64) -> Self {
				Self(value)
			}

			#[inline(always)]
			fn to_raw(self) -> u64 {
				self.0
			}
		}

		impl_descriptor_debug!($implty $name);
	};
}

define_descriptor!(table L0PageTableDescriptor, 47, 12, "L0 page table descriptor entry");

define_descriptor!(table L1PageTableDescriptor, 47, 12, "L1 page table descriptor entry");
define_descriptor!(table L2PageTableDescriptor, 47, 12, "L2 page table descriptor entry");

define_descriptor!(block L1PageTableBlockDescriptor, 47, 30, "L1 page table block entry");
define_descriptor!(block L2PageTableBlockDescriptor, 47, 21, "L2 page table block entry");
define_descriptor!(block L3PageTableBlockDescriptor, 47, 12, "L3 page table block entry");

impl<T> From<T> for PageTableEntry
where
	T: GetRaw + PageTableEntrySubtype,
{
	#[inline(always)]
	fn from(descriptor: T) -> Self {
		Self(descriptor.get_raw())
	}
}

trait GetRaw: Sized {
	#[must_use]
	fn get_raw(&self) -> u64;
	#[must_use]
	fn get_raw_mut(&mut self) -> &mut u64;
}

#[cfg(feature = "unstable")]
trait GetRawConst: Sized {
	#[must_use]
	fn with(value: u64) -> Self;
	#[must_use]
	fn to_raw(self) -> u64;
}

impl GetRaw for PageTableEntry {
	#[inline(always)]
	fn get_raw(&self) -> u64 {
		self.0
	}

	#[inline(always)]
	fn get_raw_mut(&mut self) -> &mut u64 {
		&mut self.0
	}
}

#[cfg(feature = "unstable")]
impl GetRawConst for PageTableEntry {
	#[inline(always)]
	fn with(value: u64) -> Self {
		Self(value)
	}

	#[inline(always)]
	fn to_raw(self) -> u64 {
		self.0
	}
}

/// Provides access to the valid bit of a page table entry.
pub trait PageTableEntryValidAttr: GetRaw {
	/// Checks if the page table entry is valid.
	#[inline(always)]
	fn valid(&self) -> bool {
		self.get_raw() & 0b1 != 0
	}

	/// Sets the page table entry as valid.
	#[inline(always)]
	fn set_valid(&mut self) {
		*self.get_raw_mut() |= 0b1;
	}

	/// Clears the page table entry as invalid.
	#[inline(always)]
	fn clear_valid(&mut self) {
		*self.get_raw_mut() &= !0b1;
	}
}

impl<T> PageTableEntryValidAttr for T where T: GetRaw {}

/// Provides access to the valid bit of a page table entry
/// via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryValidAttrConst: GetRawConst {
	/// Replaces the valid bit of the page table entry.
	#[inline(always)]
	#[must_use]
	fn with_valid(self) -> Self {
		Self::with(self.to_raw() | 0b1)
	}
}

#[cfg(feature = "unstable")]
impl<T> const PageTableEntryValidAttrConst for T where T: PageTableEntryValidAttr + GetRawConst {}

/// Provides access to the table descriptor bits of the upper attributes
/// of page table descriptors.
pub trait PageTableEntryTableDescriptorAttr: GetRaw {
	/// Returns the [`PageTableEntryTableAccessPerm`] of the page table entry.
	#[inline(always)]
	fn table_access_permissions(&self) -> PageTableEntryTableAccessPerm {
		unsafe { core::mem::transmute(self.get_raw() & (0b11 << 61)) }
	}

	/// Sets the [`PageTableEntryTableAccessPerm`] of the page table entry.
	///
	/// Requires a course-grained TLB invalidation of
	/// any and all page table entries that may have been
	/// affected by this change (including those in subsequent
	/// levels).
	///
	/// # Safety
	/// See _D5.5 Access controls and memory region attributes_
	/// in the ARMv8-A Architecture Reference Manual (ARM DDI 0487A.a).
	#[inline(always)]
	unsafe fn set_table_access_permissions(&mut self, perm: PageTableEntryTableAccessPerm) {
		*self.get_raw_mut() = (self.get_raw() & !(0b11 << 61)) | perm as u64;
	}
}

impl PageTableEntryTableDescriptorAttr for L0PageTableDescriptor {}
impl PageTableEntryTableDescriptorAttr for L1PageTableDescriptor {}
impl PageTableEntryTableDescriptorAttr for L2PageTableDescriptor {}

/// Provides access to the next-level bits of the upper attributes
/// of page table descriptors via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryTableDescriptorAttrConst: GetRawConst {
	/// Replaces the [`PageTableEntryTableAccessPerm`] of the page table entry.
	///
	/// # Safety
	/// See [`PageTableEntryTableDescriptorAttr::set_table_access_permissions`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	#[must_use]
	unsafe fn with_table_access_permissions(self, perm: PageTableEntryTableAccessPerm) -> Self {
		Self::with((self.to_raw() & !(0b11 << 61)) | perm as u64)
	}
}

#[cfg(feature = "unstable")]
impl<T> const PageTableEntryTableDescriptorAttrConst for T where
	T: PageTableEntryTableDescriptorAttr + GetRawConst
{
}

/// Provides access to the block descriptor attributes.
pub trait PageTableEntryBlockDescriptorAttr: GetRaw {
	/// Checks if the page table entry is a contiguous block.
	#[inline(always)]
	#[must_use]
	fn contiguous(&self) -> bool {
		self.get_raw() & (1 << 52) != 0
	}

	/// Sets the page table entry as a contiguous block.
	///
	/// # Safety
	/// Caller must ensure that the page table entry is actually contiguous.
	#[inline(always)]
	unsafe fn set_contiguous(&mut self) {
		*self.get_raw_mut() |= 1 << 52;
	}

	/// Clears the page table entry as a contiguous block.
	///
	/// # Safety
	/// Caller must ensure that the page table entry is not contiguous,
	/// and that other entries will not adversely affect memory management.
	#[inline(always)]
	unsafe fn clear_contiguous(&mut self) {
		*self.get_raw_mut() &= !(1 << 52);
	}

	/// Checks if the page is **not** global.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not global**. If it is **low**,
	/// then the page **is global**.
	#[inline(always)]
	#[must_use]
	fn not_global(&self) -> bool {
		self.get_raw() & (1 << 11) != 0
	}

	/// Sets the page as **not** global.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not global**. If it is **low**,
	/// then the page **is global**.
	///
	/// By calling this method, the page is marked as **not global**,
	#[inline(always)]
	fn set_not_global(&mut self) {
		*self.get_raw_mut() |= 1 << 11;
	}

	/// Clears the page as **not** global.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not global**. If it is **low**,
	/// then the page **is global**.
	///
	/// By calling this method, the page is marked as **global**.
	#[inline(always)]
	fn clear_not_global(&mut self) {
		*self.get_raw_mut() &= !(1 << 11);
	}

	/// Checks if the page is **not** secure.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not secure**. If it is **low**,
	/// then the page **is secure**.
	#[inline(always)]
	#[must_use]
	fn not_secure(&self) -> bool {
		self.get_raw() & (1 << 5) != 0
	}

	/// Sets the page as **not** secure.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not secure**. If it is **low**,
	/// then the page **is secure**.
	///
	/// By calling this method, the page is marked as **not secure**,
	#[inline(always)]
	fn set_not_secure(&mut self) {
		*self.get_raw_mut() |= 1 << 5;
	}

	/// Clears the page as **not** secure.
	///
	/// **NOTE:** This bit is an inverse bit; if it is **high**,
	/// then the page is **not secure**. If it is **low**,
	/// then the page **is secure**.
	///
	/// By calling this method, the page is marked as **secure**.
	#[inline(always)]
	fn clear_not_secure(&mut self) {
		*self.get_raw_mut() &= !(1 << 5);
	}

	/// Gets the access flag of the block entry.
	///
	/// **NOTE:** This entry is not held in the TLB if it is set to `0`.
	///
	/// Important note from the ARMv8-A Architecture Reference Manual:
	///
	/// > The Access flag mechanism expects that, when an Access flag fault occurs,
	/// > software resets the Access flag to 1 in the translation table entry that
	/// > caused the fault. This prevents the fault occurring the next time that
	/// > memory location is accessed. Entries with the Access flag set to 0 are
	/// > never held in the TLB, meaning software does not have to flush the entry
	/// > from the TLB after setting the flag.
	#[inline(always)]
	#[must_use]
	fn accessed(&self) -> bool {
		self.get_raw() & (1 << 10) != 0
	}

	/// Sets the access flag of the block entry.
	///
	/// **You probably don't want to be setting this manually.**
	///
	/// **NOTE:** This entry is not held in the TLB if it is set to `0`.
	///
	/// See [`PageTableEntryBlockDescriptorAttr::accessed`] for more information
	/// regarding the proper management of this bit.
	///
	/// # Safety
	/// Caller must ensure that the page table entry is properly managed.
	///
	/// **You probably don't want to be setting this manually.** Make sure to
	/// understand the effects of setting this bit before using it.
	#[inline(always)]
	unsafe fn set_accessed(&mut self) {
		*self.get_raw_mut() |= 1 << 10;
	}

	/// Clears the access flag of the block entry.
	///
	/// **NOTE:** This entry is not held in the TLB if it is set to `0`.
	///
	/// See [`PageTableEntryBlockDescriptorAttr::accessed`] for more information
	/// regarding the proper management of this bit.
	///
	/// # Safety
	/// Caller must ensure that the page table entry is properly managed.
	/// Namely, clearing this bit probably means that the page table entry
	/// was held in the TLB and thus the TLB entry should be invalidated.
	#[inline(always)]
	unsafe fn clear_accessed(&mut self) {
		*self.get_raw_mut() &= !(1 << 10);
	}

	/// Gets the MAIR index of the block entry.
	#[inline(always)]
	fn mair_index(&self) -> u64 {
		(self.get_raw() & (0b111 << 2)) >> 2
	}

	/// Sets the MAIR index of the block entry,
	/// without masking bits.
	///
	/// **You probably shouldn't use this method unless
	/// you're passing a literal or can guarantee that the
	/// MAIR index is `0..=7`.**
	///
	/// # Safety
	/// Caller must ensure that the MAIR index refers to
	/// a properly set up MAIR register attribute.
	///
	/// Further, caller must NOT pass a value above 7.
	#[inline(always)]
	unsafe fn set_mair_index_unchecked(&mut self, index: u64) {
		unsafe_precondition!(crate::Aarch64, index <= 7, "index must be 0..=7");
		*self.get_raw_mut() = (self.get_raw() & !(0b111 << 2)) | (index << 2);
	}

	/// Sets the MAIR index of the block entry.
	///
	/// Values above 7 are masked to the lowest 3 bits.
	#[inline(always)]
	fn set_mair_index(&mut self, index: u64) {
		unsafe { self.set_mair_index_unchecked(index & 0b111) }
	}

	/// Retrieves the block access permissions.
	#[inline(always)]
	#[must_use]
	fn block_access_permissions(&self) -> PageTableEntryBlockAccessPerm {
		unsafe { core::mem::transmute(self.get_raw() & (0b11 << 6)) }
	}

	/// Sets the block access permissions.
	#[inline(always)]
	fn set_block_access_permissions(&mut self, perm: PageTableEntryBlockAccessPerm) {
		*self.get_raw_mut() = (self.get_raw() & !(0b11 << 6)) | perm as u64;
	}
}

impl PageTableEntryBlockDescriptorAttr for L1PageTableBlockDescriptor {}
impl PageTableEntryBlockDescriptorAttr for L2PageTableBlockDescriptor {}
impl PageTableEntryBlockDescriptorAttr for L3PageTableBlockDescriptor {}

/// Provides access to the block descriptor attributes
/// via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryBlockDescriptorAttrConst: GetRawConst {
	/// Replaces the contiguous bit of the page table entry.
	///
	/// # Safety
	/// Caller must ensure that the page table entry is actually contiguous.
	#[inline(always)]
	#[must_use]
	unsafe fn with_contiguous(self) -> Self {
		Self::with(self.to_raw() | 1 << 52)
	}

	/// Replaces the not-global bit of the page table entry.
	///
	/// See [`PageTableEntryBlockDescriptorAttr::set_not_global`] for more information.
	#[inline(always)]
	#[must_use]
	fn with_not_global(self) -> Self {
		Self::with(self.to_raw() | 1 << 11)
	}

	/// Replaces the not-secure bit of the page table entry.
	///
	/// See [`PageTableEntryBlockDescriptorAttr::set_not_secure`] for more information.
	#[inline(always)]
	#[must_use]
	fn with_not_secure(self) -> Self {
		Self::with(self.to_raw() | 1 << 5)
	}

	/// Replaces the MAIR index of the page table entry.
	///
	/// Values above 7 are masked to the lowest 3 bits.
	#[inline(always)]
	#[must_use]
	fn with_mair_index(self, index: u64) -> Self {
		Self::with((self.to_raw() & !(0b111 << 2)) | ((index & 0b111) << 2))
	}

	/// Replaces the block acess permissions of the page table entry.
	#[inline(always)]
	#[must_use]
	fn with_block_access_permissions(self, perm: PageTableEntryBlockAccessPerm) -> Self {
		Self::with((self.to_raw() & !(0b11 << 6)) | perm as u64)
	}
}

#[cfg(feature = "unstable")]
impl<T> const PageTableEntryBlockDescriptorAttrConst for T where
	T: PageTableEntryBlockDescriptorAttr + GetRawConst
{
}

/// Provides access to the address bits of a page table entry.
pub trait PageTableEntryAddress: GetRaw + PageTableEntrySubtype {
	/// Returns the address of the page table entry.
	#[inline(always)]
	fn address(&self) -> u64 {
		self.get_raw() & Self::ADDR_MASK
	}

	/// Sets the address of the page table entry.
	///
	/// # Safety
	/// Caller must ensure that the address is properly aligned (masked).
	/// Requires a TLB entry flush of the affected page table entry/subsequent
	/// entries.
	///
	/// **NOTE:** The extra bitwise AND operation provided by [`PageTableEntryAddress::set_address`]
	/// is probably cheap enough to use in all cases, so its use is recommended unless you're
	/// _absolutely sure_ that the address is properly aligned.
	#[inline(always)]
	unsafe fn set_address_unchecked(&mut self, address: u64) {
		unsafe_precondition!(
			crate::Aarch64,
			address & !Self::ADDR_MASK == 0,
			"address must be properly aligned"
		);

		*self.get_raw_mut() = (self.get_raw() & !Self::ADDR_MASK) | address;
	}

	/// Sets the address of the page table entry.
	#[inline(always)]
	fn set_address(&mut self, address: u64) {
		unsafe { self.set_address_unchecked(address & Self::ADDR_MASK) }
	}
}

impl<T> PageTableEntryAddress for T where T: PageTableEntrySubtype + GetRaw {}

/// Provides access to the address bits of a page table entry
/// via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryAddressConst: GetRawConst + PageTableEntrySubtype {
	/// Replaces the address of the page table entry.
	#[inline(always)]
	#[must_use]
	fn with_address(self, address: u64) -> Self {
		Self::with((self.to_raw() & !Self::ADDR_MASK) | (address & Self::ADDR_MASK))
	}
}

#[cfg(feature = "unstable")]
impl<T> const PageTableEntryAddressConst for T where T: PageTableEntrySubtype + GetRawConst {}

/// Provides access to the no-execute bits of a page table entry.
pub trait PageTableEntryNoExecAttr: GetRaw {
	/// The bit offset of the unprivileged (EL0) no-execute bit (UXN).
	const NO_EXEC_USER_BIT_OFFSET: u64;
	/// The bit offset of the privileged (EL1) no-execute bit (PXN).
	const NO_EXEC_KERNEL_BIT_OFFSET: u64;

	/// Checks if the unprivileged (EL0) no-execute bit is set.
	/// If true, translations made during instruction fetching
	/// in the EL0 privilege level will fail.
	#[must_use]
	#[inline(always)]
	fn user_no_exec(&self) -> bool {
		self.get_raw() & (1 << Self::NO_EXEC_USER_BIT_OFFSET) != 0
	}

	/// Sets the unprivileged (EL0) no-execute bit.
	///
	/// Requires a course-grained TLB invalidation of
	/// any and all page table entries that may have been
	/// affected by this change (including those in subsequent
	/// levels).
	///
	/// # Safety
	/// See _D5.5 Access controls and memory region attributes_
	/// in the ARMv8-A Architecture Reference Manual (ARM DDI 0487A.a).
	#[inline(always)]
	unsafe fn set_user_no_exec(&mut self) {
		*self.get_raw_mut() |= 1 << Self::NO_EXEC_USER_BIT_OFFSET;
	}

	/// Clears the unprivileged (EL0) no-execute bit.
	///
	/// # Safety
	/// See [`PageTableEntryNoExecAttr::set_user_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	unsafe fn clear_user_no_exec(&mut self) {
		*self.get_raw_mut() &= !(1 << Self::NO_EXEC_USER_BIT_OFFSET);
	}

	/// Checks if the privileged (EL1) no-execute bit is set.
	/// If true, translations made during instruction fetching
	/// in the EL1 privilege level will fail.
	///
	/// Note that unprivileged (EL0) access is not affected by this bit.
	#[inline(always)]
	#[must_use]
	fn kernel_no_exec(&self) -> bool {
		self.get_raw() & (1 << Self::NO_EXEC_KERNEL_BIT_OFFSET) != 0
	}

	/// Sets the privileged (EL1) no-execute bit.
	/// Note that unprivileged (EL0) access is not affected by this bit.
	///
	/// # Safety
	/// Requires a course-grained TLB invalidation of
	/// any and all page table entries that may have been
	/// affected by this change (including those in subsequent
	/// levels).
	#[inline(always)]
	unsafe fn set_kernel_no_exec(&mut self) {
		*self.get_raw_mut() |= 1 << Self::NO_EXEC_KERNEL_BIT_OFFSET;
	}

	/// Clears the privileged (EL1) no-execute bit.
	/// Note that unprivileged (EL0) access is not affected by this bit.
	///
	/// # Safety
	/// See [`PageTableEntryNoExecAttr::set_kernel_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	unsafe fn clear_kernel_no_exec(&mut self) {
		*self.get_raw_mut() &= !(1 << Self::NO_EXEC_KERNEL_BIT_OFFSET);
	}
}

// FIXME(qix-): Remove this when trait negations ever land.
// FIXME(qix-): https://github.com/rust-lang/rust/issues/42721
macro_rules! impl_no_exec {
	(user_offset = $user_off:literal, kernel_offset = $kernel_off:literal, $( $t:ty ),*) => {
		$(impl PageTableEntryNoExecAttr for $t {
			const NO_EXEC_USER_BIT_OFFSET: u64 = $user_off;
			const NO_EXEC_KERNEL_BIT_OFFSET: u64 = $kernel_off;
		})*
	};
}

impl_no_exec!(
	user_offset = 60,
	kernel_offset = 59,
	L0PageTableDescriptor,
	L1PageTableDescriptor,
	L2PageTableDescriptor
);
impl_no_exec!(
	user_offset = 54,
	kernel_offset = 53,
	L1PageTableBlockDescriptor,
	L2PageTableBlockDescriptor,
	L3PageTableBlockDescriptor
);

/// Provides access to the no-execute bits of a page table entry
/// via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryNoExecAttrConst: GetRawConst + PageTableEntryNoExecAttr {
	/// Replaces the unprivileged (EL0) no-execute bit of the page table entry.
	///
	/// # Safety
	/// See [`PageTableEntryNoExecAttr::set_user_no_exec`] for information
	/// about proper TLB invalidation.
	#[must_use]
	unsafe fn with_user_no_exec(self) -> Self {
		Self::with(self.to_raw() | (1 << Self::NO_EXEC_USER_BIT_OFFSET))
	}

	/// Replaces the privileged (EL1) no-execute bit of the page table entry.
	/// Note that unprivileged (EL0) access is not affected by this bit.
	///
	/// # Safety
	/// See [`PageTableEntryNoExecAttr::set_kernel_no_exec`] for information
	/// about proper TLB invalidation.
	#[must_use]
	unsafe fn with_kernel_no_exec(self) -> Self {
		Self::with(self.to_raw() | (1 << Self::NO_EXEC_KERNEL_BIT_OFFSET))
	}
}

#[cfg(feature = "unstable")]
const _: () = {
	impl const PageTableEntryNoExecAttrConst for L0PageTableDescriptor {}
	impl const PageTableEntryNoExecAttrConst for L1PageTableDescriptor {}
	impl const PageTableEntryNoExecAttrConst for L2PageTableDescriptor {}
	impl const PageTableEntryNoExecAttrConst for L1PageTableBlockDescriptor {}
	impl const PageTableEntryNoExecAttrConst for L2PageTableBlockDescriptor {}
	impl const PageTableEntryNoExecAttrConst for L3PageTableBlockDescriptor {}
};

/// Access protection bits for a page table descriptor entry.
/// These permissions are adhered to even if subsequent
/// levels have less restrictive permissions.
///
/// Note that these are different from the AP flags
/// for block entry access permission bits ([`PageTableEntryBlockAccessPerm`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PageTableEntryTableAccessPerm {
	/// No effect on subsequent lookups
	NoEffect = 0b00 << 61,
	/// No access from EL0 (kernel only)
	KernelOnly = 0b01 << 61,
	/// Read-only, but accessible from EL0
	ReadOnly = 0b10 << 61,
	/// Read-only, but not accessible from EL0 (kernel only)
	KernelReadOnly = 0b11 << 61,
}

impl PageTableEntryTableAccessPerm {
	/// Returns the default access permissions of the page table entry.
	#[inline(always)]
	#[must_use]
	pub const fn default_const() -> Self {
		Self::NoEffect
	}
}

impl Default for PageTableEntryTableAccessPerm {
	#[inline(always)]
	fn default() -> Self {
		Self::default_const()
	}
}

/// Shareability of normal memory pages (stage 1).
///
/// More information:
/// <https://developer.arm.com/documentation/den0024/a/Memory-Ordering/Memory-attributes/Cacheable-and-shareable-memory-attributes>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PageTableEntryShareability {
	/// Non-shareable
	None = 0b00 << 8,
	/// Outer shareable
	Outer = 0b10 << 8,
	/// Inner shareable (the default)
	Inner = 0b11 << 8,
}

impl PageTableEntryShareability {
	/// Returns the default shareability of the page table entry.
	#[inline(always)]
	#[must_use]
	pub const fn default_const() -> Self {
		Self::Inner
	}
}

impl Default for PageTableEntryShareability {
	#[inline(always)]
	fn default() -> Self {
		Self::default_const()
	}
}

/// Access protection bits for a paget table block entry.
/// These permissions are adhered to even if subsequent
/// levels have less restrictive permissions.
///
/// Note that these are different from the AP flags
/// for table descriptor entry access permission bits
/// ([`PageTableEntryTableAccessPerm`]).
///
/// # Safety
/// By default, all memory is marked as inaccessible from EL0
/// (unprivileged code), but read/write from EL1 (kernel code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum PageTableEntryBlockAccessPerm {
	/// EL1 (kernel) read/write, EL0 (user) no access
	KernelRWUserNoAccess = 0b00 << 6,
	/// EL1 (kernel) read/write, EL0 (user) read/write
	KernelRWUserRW = 0b01 << 6,
	/// EL1 (kernel) read-only, EL0 (user) no access
	KernelROUserNoAccess = 0b10 << 6,
	/// EL1 (kernel) read-only, EL0 (user) read-only
	KernelROUserRO = 0b11 << 6,
}

impl PageTableEntryBlockAccessPerm {
	/// Returns the default access permissions of the page table entry.
	#[inline(always)]
	#[must_use]
	pub const fn default_const() -> Self {
		Self::KernelRWUserNoAccess
	}
}

impl Default for PageTableEntryBlockAccessPerm {
	#[inline(always)]
	fn default() -> Self {
		Self::default_const()
	}
}
