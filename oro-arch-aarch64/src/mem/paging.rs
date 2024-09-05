//! Aarch64 page table structures and manipulators.
//!
//! # Notes
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
//! Note that the address and type bits encoding for a [`L3PageTableBlockDescriptor`]
//! _is the same†_ as [`L0PageTableDescriptor`], [`L1PageTableDescriptor`] and
//! [`L2PageTableDescriptor`], but the semantics of the address bits is different.
//!
//! <sub>†: See [Controlling address translation - Translation table format](https://developer.arm.com/documentation/101811/0103/Controlling-address-translation-Translation-table-format)
//! under final _Note_ block.</sub>

// NOTE(qix-): This used to use a slew of traits and whatnot to keep things a lot cleaner,
// NOTE(qix-): but ultimately fell apart due to a buggy const traits implementation in Rust
// NOTE(qix-): (granted, at the time of writing, it's an unstable feature). Since these types
// NOTE(qix-): *really do* need to be constructed in a constant context in certain parts of the
// NOTE(qix-): kernel, I've opted to use a macro-based approach to keep things clean and
// NOTE(qix-): maintainable. This is a bit of a compromise, but it's the best we can do for now.
// NOTE(qix-):
// NOTE(qix-): In the event const traits ever get fixed and stabilized, feel free to submit a PR
// NOTE(qix-): to refactor this to use const traits instead. Check the commit log for around
// NOTE(qix-): 18-20 June 2024 to see what this looked like before, for inspiration.

// TODO(qix-): Very much not happy with how this is structured. It's way too rigid and will be
// TODO(qix-): nearly impossible to maintain or extend in the future. It needs a full rewrite.

#![allow(clippy::inline_always, private_bounds)]

use core::{
	fmt,
	ops::{Index, IndexMut},
};
use oro_macro::assert;

/// A single page table entry.
#[derive(Debug, Clone)]
#[repr(C, align(4096))]
pub struct PageTable {
	/// The underlying page table entries.
	///
	/// # Safety
	/// Under certin granule sizes, the page table entry
	/// count changes. This type will need to be adjusted
	/// accordingly in the future.
	entries: [PageTableEntry; 512],
}

const _: () = assert::size_of::<PageTable, 4096>();

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

	/// Shallow copies the given [`PageTable`] into this
	/// page table. This copies the top level entries to this
	/// one, without recursively allocating and copying the
	/// higher (deeper) levels.
	pub fn shallow_copy_from(&mut self, other: &PageTable) {
		self.entries.copy_from_slice(&other.entries);
	}

	/// Returns whether or not the page table is empty (all entries are invalid).
	pub fn empty(&self) -> bool {
		self.entries.iter().all(|entry| !entry.valid())
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
	Invalid(&'a PageTableEntry),
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
	Malformed(&'a PageTableEntry),
	/// An L0 page table descriptor entry.
	L0Descriptor(&'a L0PageTableDescriptor),
	/// An L1 page table descriptor entry.
	L1Descriptor(&'a L1PageTableDescriptor),
	/// An L2 page table descriptor entry.
	L2Descriptor(&'a L2PageTableDescriptor),
	/// An L1 page table block entry.
	L1Block(&'a L1PageTableBlockDescriptor),
	/// An L2 page table block entry.
	L2Block(&'a L2PageTableBlockDescriptor),
	/// An L3 page table block entry.
	L3Block(&'a L3PageTableBlockDescriptor),
}

/// Describes the type of a page table entry, based on its level.
/// Holds a mutable reference to the entry.
#[derive(Debug)]
pub enum PageTableEntryTypeMut<'a> {
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

#[allow(clippy::missing_docs_in_private_items)]
const _: () = assert::size_of::<PageTableEntry, 8>();

macro_rules! impl_page_table_entry_type {
	($level:expr, $self:expr, $from:ident, $to:ident, $EntryType:ty) => {{
		debug_assert!($level <= 3, "level must be 0..=3");

		if !$self.valid() {
			return <$EntryType>::Invalid($self);
		}

		match $level {
			0 => {
				if $self.table() {
					<$EntryType>::L0Descriptor(
						core::ptr::$from($self)
							.cast::<L0PageTableDescriptor>()
							.$to(),
					)
				} else {
					<$EntryType>::Malformed($self)
				}
			}
			1 => {
				if $self.table() {
					<$EntryType>::L1Descriptor(
						core::ptr::$from($self)
							.cast::<L1PageTableDescriptor>()
							.$to(),
					)
				} else {
					<$EntryType>::L1Block(
						core::ptr::$from($self)
							.cast::<L1PageTableBlockDescriptor>()
							.$to(),
					)
				}
			}
			2 => {
				if $self.table() {
					<$EntryType>::L2Descriptor(
						core::ptr::$from($self)
							.cast::<L2PageTableDescriptor>()
							.$to(),
					)
				} else {
					<$EntryType>::L2Block(
						core::ptr::$from($self)
							.cast::<L2PageTableBlockDescriptor>()
							.$to(),
					)
				}
			}
			3 => {
				// NOTE(qix-): This might look incorrect, but it's not.
				// NOTE(qix-): The "table" bit is set for L3 block entries.
				// NOTE(qix-): Bits [1:0] == 0b01 for L3 block entries is considered
				// NOTE(qix-): a "malformed" (reserved) bit representation and is treated
				// NOTE(qix-): as an invalid entry by the translation table walk.
				// NOTE(qix-):
				// NOTE(qix-): Check D5.4.2 of the ARMv8-A Architecture Reference Manual.
				if $self.table() {
					<$EntryType>::L3Block(
						core::ptr::$from($self)
							.cast::<L3PageTableBlockDescriptor>()
							.$to(),
					)
				} else {
					<$EntryType>::Malformed($self)
				}
			}
			_ => unreachable!(),
		}
	}};
}

impl PageTableEntry {
	/// Creates a new page table entry.
	#[allow(clippy::new_without_default)]
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
	pub fn table(self) -> bool {
		self.raw() & 0b1 << 1 != 0
	}

	/// Sets the page table entry as a table descriptor.
	///
	/// # Safety
	/// Caller must ensure that the bitwise representation
	/// of the page table entry is correct for a table descriptor,
	/// or that the entry is otherwise well-formed or unused until
	/// properly initialized.
	#[inline(always)]
	pub unsafe fn set_table(&mut self) {
		*self.raw_mut() |= 0b1 << 1;
	}

	/// Clears the page table entry as a block descriptor.
	///
	/// # Safety
	/// Caller must ensure this is not being called on an
	/// otherwise well-formed L0 or L3 table descriptor,
	/// as this will result in a malformed entry.
	#[inline(always)]
	pub unsafe fn clear_table(&mut self) {
		*self.raw_mut() &= !(0b1 << 1);
	}

	/// Replaces the page table entry as a table descriptor.
	#[inline(always)]
	#[must_use]
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
	pub unsafe fn address(&self, level: u8) -> Option<u64> {
		debug_assert!(level <= 3, "level must be 0..=3");

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
	pub unsafe fn entry_type(&self, level: u8) -> PageTableEntryType {
		impl_page_table_entry_type!(level, self, from_ref, as_ref_unchecked, PageTableEntryType)
	}

	/// Returns the type of the page table entry based
	/// on the level of the page table. Returns a mutable
	/// reference to the entry.
	///
	/// # Safety
	/// Caller must ensure that `level` is `0..=3` and that it
	/// is correctly specified. **Do not assume this value.**
	#[inline]
	#[must_use]
	pub unsafe fn entry_type_mut(&mut self, level: u8) -> PageTableEntryTypeMut {
		impl_page_table_entry_type!(
			level,
			self,
			from_mut,
			as_mut_unchecked,
			PageTableEntryTypeMut
		)
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

/// Calculates an initial value for page table entries based
/// on the type of the descriptor (block vs table).
macro_rules! descriptor_init_value {
	(table) => {
		!0b1 & (
			0b10
				| PageTableEntryTableAccessPerm::default_const() as u64
		)
	};
	(block) => {
		!0b1 & (
			PageTableEntryShareability::default_const() as u64
				| PageTableEntryBlockAccessPerm::default_const() as u64
				// Access flag (AF=1)
				| (1 << 10)
		)
	};
	// NOTE(qix-): L3 block descriptors must have the normally indicative "table" bit set.
	// NOTE(qix-): This might look wrong, but it's not.
	// NOTE(qix-):
	// NOTE(qix-): Check D5.4.2 of the ARMv8-A Architecture Reference Manual.
	(l3) => {
		!0b1 & (
			0b10
				| PageTableEntryShareability::default_const() as u64
				| PageTableEntryBlockAccessPerm::default_const() as u64
				// Access flag (AF=1)
				| (1 << 10)
		)
	};
}

// FIXME(qix-): Workaround for a rustfmt bug where, when inlined
// FIXME(qix-): with the #[doc = ...] attribute on the subtype's
// FIXME(qix-): ::new() function, the doc comment keeps getting
// FIXME(qix-): indented whenever rustfmt runs.
#[allow(clippy::missing_docs_in_private_items)]
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

/// Implements the `Debug` trait for a descriptor type.
macro_rules! impl_descriptor_debug {
	(table $name:ty) => {
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

	(block $name:ty) => {
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

/// Defines a new page table descriptor type and implementations for a specific level and descriptor type.
macro_rules! define_descriptor {
	($implty:tt $dbgty:tt $name:ident, $addr_mask_high:expr, $addr_mask_low:expr, $doc:literal) => {
		#[doc = concat!("An ", $doc, ".")]
		#[derive(Clone, Copy)]
		#[repr(C, align(8))]
		pub struct $name(u64);

		const _: () = assert::size_of::<$name, 8>();

		impl PageTableEntrySubtype for $name {
			const ADDR_MASK_HIGH_BIT: u64 = $addr_mask_high;
			const ADDR_MASK_LOW_BIT: u64 = $addr_mask_low;
		}

		// TODO(qix-) Add docs. Silencing for now as it's inflating compile times.
		#[allow(missing_docs)]
		impl $name {
			#[allow(clippy::new_without_default)]
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

			#[inline(always)]
			pub fn raw(&self) -> u64 {
				self.0
			}

			#[inline(always)]
			pub fn raw_mut(&mut self) -> &mut u64 {
				&mut self.0
			}

			#[inline(always)]
			pub fn set_raw(&mut self, raw: u64) {
				*self.raw_mut() = raw;
			}

			#[inline(always)]
			pub const fn with(value: u64) -> Self {
				Self(value)
			}

			#[inline(always)]
			pub const fn to_raw(self) -> u64 {
				self.0
			}

			#[inline(always)]
			pub const fn to_entry(self) -> PageTableEntry {
				PageTableEntry(self.to_raw())
			}
		}

		impl From<$name> for PageTableEntry {
			#[inline(always)]
			fn from(descriptor: $name) -> Self {
				Self(descriptor.raw())
			}
		}

		impl_descriptor_debug!($dbgty $name);
	};
}

define_descriptor!(table table L0PageTableDescriptor, 47, 12, "L0 page table descriptor entry");

define_descriptor!(table table L1PageTableDescriptor, 47, 12, "L1 page table descriptor entry");
define_descriptor!(table table L2PageTableDescriptor, 47, 12, "L2 page table descriptor entry");

define_descriptor!(block block L1PageTableBlockDescriptor, 47, 30, "L1 page table block entry");
define_descriptor!(block block L2PageTableBlockDescriptor, 47, 21, "L2 page table block entry");
define_descriptor!(l3 block L3PageTableBlockDescriptor, 47, 12, "L3 page table block entry");

impl PageTableEntry {
	/// Returns the raw value of the page table entry.
	#[inline(always)]
	pub fn raw(self) -> u64 {
		self.0
	}

	/// Returns the raw value of the page table entry as a mutable reference.
	///
	/// # Safety
	/// Values can be mutated in-place, but care must be taken to ensure
	/// that the page table entry is not corrupted.
	#[inline(always)]
	pub unsafe fn raw_mut(&mut self) -> &mut u64 {
		&mut self.0
	}

	/// Creates a new page table entry at build time with the given
	/// raw value.
	///
	/// # Safety
	/// The value must be a well-formed page table entry.
	#[inline(always)]
	pub const unsafe fn with(value: u64) -> Self {
		Self(value)
	}

	/// Returns the raw value of the page table entry
	/// as a constant value.
	#[inline(always)]
	pub const fn to_raw(self) -> u64 {
		self.0
	}

	/// Sets the raw value of the page table entry.
	///
	/// # Safety
	/// The value must be a well-formed page table entry.
	#[inline(always)]
	pub unsafe fn set_raw(&mut self, raw: u64) {
		*self.raw_mut() = raw;
	}
}

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_page_table_entry_valid_attr {
	($($name:ty),*) => {
		$(impl $name {
			/// Checks if the page table entry is valid.
			#[inline(always)]
			pub fn valid(&self) -> bool {
				self.raw() & 0b1 != 0
			}

			/// Sets the page table entry as valid.
			///
			/// # Safety
			/// This effectively 'enables' the page table entry.
			/// Caller must ensure that the page table entry is well-formed.
			#[inline(always)]
			pub unsafe fn set_valid(&mut self) {
				*self.raw_mut() |= 0b1;
			}

			/// Clears the page table entry as invalid.
			#[inline(always)]
			pub fn clear_valid(&mut self) {
				// SAFETY TODO(qix-): Not sure why this triggers an "unnecessary unsafe block" warning when removed.
				#[allow(unused_unsafe)]
				unsafe { *self.raw_mut() &= !0b1; }
			}

			/// Replaces the valid bit of the page table entry with a `1`.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is well-formed
			/// prior to use.
			#[inline(always)]
			#[must_use]
			pub const unsafe fn with_valid(self) -> Self {
				Self::with(self.to_raw() | 0b1)
			}
		})*
	};
}

impl_page_table_entry_valid_attr!(
	PageTableEntry,
	L0PageTableDescriptor,
	L1PageTableBlockDescriptor,
	L1PageTableDescriptor,
	L2PageTableBlockDescriptor,
	L2PageTableDescriptor,
	L3PageTableBlockDescriptor
);

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_page_table_entry_table_descriptor_attr {
	($($name:ty),*) => {
		$(impl $name {
			/// Returns the [`PageTableEntryTableAccessPerm`] of the page table entry.
			#[inline(always)]
			pub fn table_access_permissions(&self) -> PageTableEntryTableAccessPerm {
				unsafe { core::mem::transmute(self.raw() & (0b11 << 61)) }
			}

			/// Sets the [`PageTableEntryTableAccessPerm`] of the page table entry.
			///
			/// Requires a course-grained TLB invalidation of
			/// any and all page table entries that may have been
			/// affected by this change (including those in subsequent
			/// levels).
			///
			/// # Safety
			/// See D5.5 Access controls and memory region attributes_
			/// in the ARMv8-A Architecture Reference Manual (ARM DDI 0487A.a).
			#[inline(always)]
			pub unsafe fn set_table_access_permissions(&mut self, perm: PageTableEntryTableAccessPerm) {
				*self.raw_mut() = (self.raw() & !(0b11 << 61)) | perm as u64;
			}

			/// Replaces the [`PageTableEntryTableAccessPerm`] of the page table entry.
			///
			/// # Safety
			/// See [`Self::set_table_access_permissions()`] for information
			/// about proper TLB invalidation.
			#[inline(always)]
			#[must_use]
			pub const unsafe fn with_table_access_permissions(self, perm: PageTableEntryTableAccessPerm) -> Self {
				Self::with((self.to_raw() & !(0b11 << 61)) | perm as u64)
			}
		})*
	}
}

impl_page_table_entry_table_descriptor_attr!(
	L0PageTableDescriptor,
	L1PageTableDescriptor,
	L2PageTableDescriptor
);

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_page_table_entry_block_descriptor_attr {
	($($name:ty),*) => {
		$(impl $name {
			/// Checks if the page table entry is a contiguous block.
			#[inline(always)]
			#[must_use]
			pub fn contiguous(&self) -> bool {
				self.raw() & (1 << 52) != 0
			}

			/// Sets the page table entry as a contiguous block.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is actually contiguous.
			#[inline(always)]
			pub unsafe fn set_contiguous(&mut self) {
				*self.raw_mut() |= 1 << 52;
			}

			/// Clears the page table entry as a contiguous block.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is not contiguous,
			/// and that other entries will not adversely affect memory management.
			#[inline(always)]
			pub unsafe fn clear_contiguous(&mut self) {
				*self.raw_mut() &= !(1 << 52);
			}

			/// Checks if the page is **not** global.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not global**. If it is **low**,
			/// then the page **is global**.
			#[inline(always)]
			#[must_use]
			pub fn not_global(&self) -> bool {
				self.raw() & (1 << 11) != 0
			}

			/// Sets the page as **not** global.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not global**. If it is **low**,
			/// then the page **is global**.
			///
			/// By calling this method, the page is marked as **not global**,
			#[inline(always)]
			pub fn set_not_global(&mut self) {
				*self.raw_mut() |= 1 << 11;
			}

			/// Clears the page as **not** global.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not global**. If it is **low**,
			/// then the page **is global**.
			///
			/// By calling this method, the page is marked as **global**.
			#[inline(always)]
			pub fn clear_not_global(&mut self) {
				*self.raw_mut() &= !(1 << 11);
			}

			/// Checks if the page is **not** secure.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not secure**. If it is **low**,
			/// then the page **is secure**.
			#[inline(always)]
			#[must_use]
			pub fn not_secure(&self) -> bool {
				self.raw() & (1 << 5) != 0
			}

			/// Sets the page as **not** secure.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not secure**. If it is **low**,
			/// then the page **is secure**.
			///
			/// By calling this method, the page is marked as **not secure**,
			#[inline(always)]
			pub fn set_not_secure(&mut self) {
				*self.raw_mut() |= 1 << 5;
			}

			/// Clears the page as **not** secure.
			///
			/// **NOTE:** This bit is an inverse bit; if it is **high**,
			/// then the page is **not secure**. If it is **low**,
			/// then the page **is secure**.
			///
			/// By calling this method, the page is marked as **secure**.
			#[inline(always)]
			pub fn clear_not_secure(&mut self) {
				*self.raw_mut() &= !(1 << 5);
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
			pub fn accessed(&self) -> bool {
				self.raw() & (1 << 10) != 0
			}

			/// Sets the access flag of the block entry.
			///
			/// **NOTE:** This entry is not held in the TLB if it is set to `0`.
			///
			/// See [`Self::accessed()`] for more information
			/// regarding the proper management of this bit.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is properly managed.
			#[inline(always)]
			pub unsafe fn set_accessed(&mut self) {
				*self.raw_mut() |= 1 << 10;
			}

			/// Clears the access flag of the block entry.
			///
			/// **NOTE:** This entry is not held in the TLB if it is set to `0`.
			///
			/// See [`Self::accessed()`] for more information
			/// regarding the proper management of this bit.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is properly managed.
			/// Namely, clearing this bit probably means that the page table entry
			/// was held in the TLB and thus the TLB entry should be invalidated.
			#[inline(always)]
			pub unsafe fn clear_accessed(&mut self) {
				*self.raw_mut() &= !(1 << 10);
			}

			/// Gets the MAIR index of the block entry.
			#[inline(always)]
			pub fn mair_index(&self) -> u64 {
				(self.raw() & (0b111 << 2)) >> 2
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
			pub unsafe fn set_mair_index_unchecked(&mut self, index: u64) {
				debug_assert!(index <= 7, "index must be 0..=7");
				*self.raw_mut() = (self.raw() & !(0b111 << 2)) | (index << 2);
			}

			/// Sets the MAIR index of the block entry.
			///
			/// Values above 7 are masked to the lowest 3 bits.
			#[inline(always)]
			pub fn set_mair_index(&mut self, index: u64) {
				unsafe { self.set_mair_index_unchecked(index & 0b111) }
			}

			/// Retrieves the block access permissions.
			#[inline(always)]
			#[must_use]
			pub fn block_access_permissions(&self) -> PageTableEntryBlockAccessPerm {
				unsafe { core::mem::transmute(self.raw() & (0b11 << 6)) }
			}

			/// Sets the block access permissions.
			#[inline(always)]
			pub fn set_block_access_permissions(&mut self, perm: PageTableEntryBlockAccessPerm) {
				*self.raw_mut() = (self.raw() & !(0b11 << 6)) | perm as u64;
			}

			/// Replaces the contiguous bit of the page table entry.
			///
			/// # Safety
			/// Caller must ensure that the page table entry is actually contiguous.
			#[inline(always)]
			#[must_use]
			pub const unsafe fn with_contiguous(self) -> Self {
				Self::with(self.to_raw() | 1 << 52)
			}

			/// Replaces the not-global bit of the page table entry.
			///
			/// See [`Self::set_not_global()`] for more information.
			#[inline(always)]
			#[must_use]
			pub const fn with_not_global(self) -> Self {
				Self::with(self.to_raw() | 1 << 11)
			}

			/// Replaces the not-secure bit of the page table entry.
			///
			/// See [`Self::set_not_secure()`] for more information.
			#[inline(always)]
			#[must_use]
			pub const fn with_not_secure(self) -> Self {
				Self::with(self.to_raw() | 1 << 5)
			}

			/// Replaces the MAIR index of the page table entry.
			///
			/// Values above 7 are masked to the lowest 3 bits.
			#[inline(always)]
			#[must_use]
			pub const fn with_mair_index(self, index: u64) -> Self {
				Self::with((self.to_raw() & !(0b111 << 2)) | ((index & 0b111) << 2))
			}

			/// Replaces the block acess permissions of the page table entry.
			#[inline(always)]
			#[must_use]
			pub const fn with_block_access_permissions(self, perm: PageTableEntryBlockAccessPerm) -> Self {
				Self::with((self.to_raw() & !(0b11 << 6)) | perm as u64)
			}
		})*
	}
}

impl_page_table_entry_block_descriptor_attr!(
	L1PageTableBlockDescriptor,
	L2PageTableBlockDescriptor,
	L3PageTableBlockDescriptor
);

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_page_table_entry_address {
	($($name:ty),*) => {
		$(impl $name {
			/// Returns the address of the page table entry.
			#[inline(always)]
			pub fn address(&self) -> u64 {
				self.raw() & Self::ADDR_MASK
			}

			/// Sets the address of the page table entry.
			///
			/// # Safety
			/// Caller must ensure that the address is properly aligned (masked).
			/// Requires a TLB entry flush of the affected page table entry/subsequent
			/// entries.
			///
			/// **NOTE:** The extra bitwise AND operation provided by [`Self::set_address()`]
			/// is probably cheap enough to use in all cases, so its use is recommended unless you're
			/// _absolutely sure_ that the address is properly aligned.
			#[inline(always)]
			pub unsafe fn set_address_unchecked(&mut self, address: u64) {
				debug_assert_eq!(
					address & !Self::ADDR_MASK, 0,
					"address must be properly aligned"
				);

				*self.raw_mut() = (self.raw() & !Self::ADDR_MASK) | address;
			}

			/// Sets the address of the page table entry.
			#[inline(always)]
			pub fn set_address(&mut self, address: u64) {
				unsafe { self.set_address_unchecked(address & Self::ADDR_MASK) }
			}

			/// Replaces the address of the page table entry.
			#[inline(always)]
			#[must_use]
			pub const fn with_address(self, address: u64) -> Self {
				Self::with((self.to_raw() & !Self::ADDR_MASK) | (address & Self::ADDR_MASK))
			}
		})*
	}
}

impl_page_table_entry_address!(
	L0PageTableDescriptor,
	L1PageTableDescriptor,
	L2PageTableDescriptor,
	L1PageTableBlockDescriptor,
	L2PageTableBlockDescriptor,
	L3PageTableBlockDescriptor
);

#[allow(clippy::missing_docs_in_private_items)]
macro_rules! impl_page_table_entry_no_exec_attr {
	(user_offset = $user_off:literal, kernel_offset = $kernel_off:literal, $($name:ty),*) => {
		$(impl $name {
			/// The bit offset of the unprivileged (EL0) no-execute bit (UXN).
			pub const NO_EXEC_USER_BIT_OFFSET: u64 = $user_off;
			/// The bit offset of the privileged (EL1) no-execute bit (PXN).
			pub const NO_EXEC_KERNEL_BIT_OFFSET: u64 = $kernel_off;

			/// Checks if the unprivileged (EL0) no-execute bit is set.
			/// If true, translations made during instruction fetching
			/// in the EL0 privilege level will fail.
			#[must_use]
			#[inline(always)]
			pub fn user_no_exec(&self) -> bool {
				self.raw() & (1 << Self::NO_EXEC_USER_BIT_OFFSET) != 0
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
			pub unsafe fn set_user_no_exec(&mut self) {
				*self.raw_mut() |= 1 << Self::NO_EXEC_USER_BIT_OFFSET;
			}

			/// Clears the unprivileged (EL0) no-execute bit.
			///
			/// # Safety
			/// See [`Self::set_user_no_exec()`] for information
			/// about proper TLB invalidation.
			#[inline(always)]
			pub unsafe fn clear_user_no_exec(&mut self) {
				*self.raw_mut() &= !(1 << Self::NO_EXEC_USER_BIT_OFFSET);
			}

			/// Checks if the privileged (EL1) no-execute bit is set.
			/// If true, translations made during instruction fetching
			/// in the EL1 privilege level will fail.
			///
			/// Note that unprivileged (EL0) access is not affected by this bit.
			#[inline(always)]
			#[must_use]
			pub fn kernel_no_exec(&self) -> bool {
				self.raw() & (1 << Self::NO_EXEC_KERNEL_BIT_OFFSET) != 0
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
			pub unsafe fn set_kernel_no_exec(&mut self) {
				*self.raw_mut() |= 1 << Self::NO_EXEC_KERNEL_BIT_OFFSET;
			}

			/// Clears the privileged (EL1) no-execute bit.
			/// Note that unprivileged (EL0) access is not affected by this bit.
			///
			/// # Safety
			/// See [`Self::set_kernel_no_exec()`] for information
			/// about proper TLB invalidation.
			#[inline(always)]
			pub unsafe fn clear_kernel_no_exec(&mut self) {
				*self.raw_mut() &= !(1 << Self::NO_EXEC_KERNEL_BIT_OFFSET);
			}

			/// Replaces the unprivileged (EL0) no-execute bit of the page table entry.
			///
			/// # Safety
			/// See [`Self::set_user_no_exec()`] for information
			/// about proper TLB invalidation.
			#[must_use]
			pub const unsafe fn with_user_no_exec(self) -> Self {
				Self::with(self.to_raw() | (1 << Self::NO_EXEC_USER_BIT_OFFSET))
			}

			/// Replaces the privileged (EL1) no-execute bit of the page table entry.
			/// Note that unprivileged (EL0) access is not affected by this bit.
			///
			/// # Safety
			/// See [`Self::set_kernel_no_exec()`] for information
			/// about proper TLB invalidation.
			#[must_use]
			pub const unsafe fn with_kernel_no_exec(self) -> Self {
				Self::with(self.to_raw() | (1 << Self::NO_EXEC_KERNEL_BIT_OFFSET))
			}
		})*
	}
}

impl_page_table_entry_no_exec_attr!(
	user_offset = 60,
	kernel_offset = 59,
	L0PageTableDescriptor,
	L1PageTableDescriptor,
	L2PageTableDescriptor
);
impl_page_table_entry_no_exec_attr!(
	user_offset = 54,
	kernel_offset = 53,
	L1PageTableBlockDescriptor,
	L2PageTableBlockDescriptor,
	L3PageTableBlockDescriptor
);

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
	NoEffect       = 0b00 << 61,
	/// No access from EL0 (kernel only)
	KernelOnly     = 0b01 << 61,
	/// Read-only, but accessible from EL0
	ReadOnly       = 0b10 << 61,
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
#[allow(unused)]
pub enum PageTableEntryShareability {
	/// Non-shareable
	None  = 0b00 << 8,
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
	KernelRWUserRW       = 0b01 << 6,
	/// EL1 (kernel) read-only, EL0 (user) no access
	KernelROUserNoAccess = 0b10 << 6,
	/// EL1 (kernel) read-only, EL0 (user) read-only
	KernelROUserRO       = 0b11 << 6,
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
