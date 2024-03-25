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

use core::ops::{Index, IndexMut};
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
#[derive(Debug, Clone, Copy)]
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

macro_rules! define_descriptor {
	($name:ident, $init:expr, $addr_mask_high:expr, $addr_mask_low:expr) => {
		/// A single page table entry subtype.
		#[derive(Debug, Clone, Copy)]
		#[repr(C, align(8))]
		pub struct $name(u64);

		static_assertions::const_assert_eq!(::core::mem::size_of::<$name>(), 8);

		impl PageTableEntrySubtype for $name {
			const ADDR_MASK_HIGH_BIT: u64 = $addr_mask_high;
			const ADDR_MASK_LOW_BIT: u64 = $addr_mask_low;
		}

		impl $name {
			/// Creates a new page table subtype entry with its initialization value
			/// (different for each subtype).
			///
			/// This constructor marks the descriptor as invalid,
			/// but might set other bits as necessary.
			///
			/// # Safety
			/// Caller must ensure that the descriptor subtype is being used
			/// at the correct level in the correct manner.
			#[inline(always)]
			#[must_use]
			pub const fn new() -> Self {
				Self($init & !0b1)
			}

			/// Resets the page table entry to its initial state.
			#[inline(always)]
			pub fn reset(&mut self) {
				self.0 = $init & !0b1;
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
	};
}

define_descriptor!(L0PageTableDescriptor, 0b10, 47, 12);
define_descriptor!(L1PageTableDescriptor, 0b10, 47, 12);
define_descriptor!(L2PageTableDescriptor, 0b10, 47, 12);

define_descriptor!(L1PageTableBlockDescriptor, 0, 47, 30);
define_descriptor!(L2PageTableBlockDescriptor, 0, 47, 21);

define_descriptor!(L3PageTableBlockDescriptor, 0b10, 47, 12);

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

/// Provides access to the next-level bits of the upper attributes
/// of page table descriptors.
pub trait PageTableEntryNextLevelAttr: GetRaw {
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

	/// Checks if the unprivileged (EL0) no-execute bit is set.
	/// If true, translations made during instruction fetching
	/// in the EL0 privilege level will fail.
	#[inline(always)]
	#[must_use]
	fn user_no_exec(&self) -> bool {
		self.get_raw() & (1 << 60) != 0
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
		*self.get_raw_mut() |= 1 << 60;
	}

	/// Clears the unprivileged (EL0) no-execute bit.
	///
	/// # Safety
	/// See [`PageTableEntryNextLevelAttr::set_user_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	unsafe fn clear_user_no_exec(&mut self) {
		*self.get_raw_mut() &= !(1 << 60);
	}

	/// Checks if the privileged (EL1) no-execute bit is set.
	/// If true, translations made during instruction fetching
	/// in the EL1 privilege level will fail.
	///
	/// Note that unprivileged (EL0) access is not affected by this bit.
	#[inline(always)]
	#[must_use]
	fn kernel_no_exec(&self) -> bool {
		self.get_raw() & (1 << 59) != 0
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
		*self.get_raw_mut() |= 1 << 59;
	}

	/// Clears the privileged (EL1) no-execute bit.
	/// Note that unprivileged (EL0) access is not affected by this bit.
	///
	/// # Safety
	/// See [`PageTableEntryNextLevelAttr::set_kernel_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	unsafe fn clear_kernel_no_exec(&mut self) {
		*self.get_raw_mut() &= !(1 << 59);
	}
}

impl PageTableEntryNextLevelAttr for L0PageTableDescriptor {}
impl PageTableEntryNextLevelAttr for L1PageTableDescriptor {}
impl PageTableEntryNextLevelAttr for L2PageTableDescriptor {}

/// Provides access to the next-level bits of the upper attributes
/// of page table descriptors via `const` methods.
#[cfg(feature = "unstable")]
pub trait PageTableEntryNextLevelAttrConst: GetRawConst {
	/// Replaces the [`PageTableEntryTableAccessPerm`] of the page table entry.
	///
	/// # Safety
	/// See [`PageTableEntryNextLevelAttr::set_table_access_permissions`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	#[must_use]
	unsafe fn with_table_access_permissions(self, perm: PageTableEntryTableAccessPerm) -> Self {
		Self::with((self.to_raw() & !(0b11 << 61)) | perm as u64)
	}

	/// Replaces the unprivileged (EL0) no-execute bit of the page table entry.
	///
	/// # Safety
	/// See [`PageTableEntryNextLevelAttr::set_user_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	#[must_use]
	unsafe fn with_user_no_exec(self) -> Self {
		Self::with(self.to_raw() | (1 << 60))
	}

	/// Replaces the privileged (EL1) no-execute bit of the page table entry.
	/// Note that unprivileged (EL0) access is not affected by this bit.
	///
	/// # Safety
	/// See [`PageTableEntryNextLevelAttr::set_kernel_no_exec`] for information
	/// about proper TLB invalidation.
	#[inline(always)]
	#[must_use]
	unsafe fn with_kernel_no_exec(self) -> Self {
		Self::with(self.to_raw() | (1 << 59))
	}
}

#[cfg(feature = "unstable")]
impl<T> const PageTableEntryNextLevelAttrConst for T where
	T: PageTableEntryNextLevelAttr + GetRawConst
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
impl<T> PageTableEntryAddressConst for T where T: PageTableEntrySubtype + GetRawConst {}

/// Access protection bits for a page table entry.
/// These permissions are adhered to even if subsequent
/// levels have less restrictive permissions.
///
/// Note that these are different from the AP flags
/// for L3 block entry access permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u64)]
pub enum PageTableEntryTableAccessPerm {
	/// No effect on subsequent lookups
	#[default]
	NoEffect = 0b00 << 61,
	/// No access from EL0 (kernel only)
	KernelOnly = 0b01 << 61,
	/// Read-only, but accessible from EL0
	ReadOnly = 0b10 << 61,
	/// Read-only, but not accessible from EL0 (kernel only)
	KernelReadOnly = 0b11 << 61,
}
