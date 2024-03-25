//! **NOTE(qix-):**
//! For now, a granule size of 4KiB is assumed. This affects
//! the page table sizes, which is currently set to 512 entries.
//! In the future, this may be configurable, but will require
//! different structures for the page tables and an abstraction
//! over them (e.g. the 16KiB granule size bifurcates the address
//! space using bit 47 instead of using an L0 index).
//!
//! For future reference, check D5.2.3 of the ARMv8-A Architecture
//! Reference Manual (ARM DDI 0487A.a) for more information.

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
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		debug_assert!(index < 512, "index out of bounds (max 511)");
		&mut self.entries[index]
	}
}

impl Index<usize> for PageTable {
	type Output = PageTableEntry;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output {
		debug_assert!(index < 512, "index out of bounds (max 511)");
		&self.entries[index]
	}
}

impl PageTable {
	/// Clears all entries in the page table.
	#[inline]
	pub fn reset(&mut self) {
		for entry in &mut self.entries {
			entry.reset();
		}
	}
}

/// Describes the type of a page table entry, based on its level.
pub enum PageTableEntryType<'a> {
	/// An invalid page table entry
	Invalid(&'a mut PageTableEntry),
	/// A malformed page table entry. Returned
	/// when a level of 3 is given to
	/// [`PageTableEntry::entry_type`] but bit 1
	/// is not set.
	Malformed(&'a mut PageTableEntry),
	/// An L0/L1/L2 page table descriptor
	L012Descriptor(&'a mut L012PageTableDescriptor),
	/// An L0/L1/L2 page table block
	L012Block(&'a mut L012PageTableBlock),
	/// An L3 page table block
	L3Block(&'a mut L3PageTableBlock),
}

/// A single page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PageTableEntry(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<PageTableEntry>(), 8);

impl PageTableEntry {
	/// Creates a new page table entry.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Resets the page table entry to its default state.
	#[inline]
	pub fn reset(&mut self) {
		self.0 = 0;
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

		if self.0 & 0b1 == 0 {
			return PageTableEntryType::Invalid(self);
		}

		match level {
			0..=2 => {
				if self.0 & 0b10 == 0 {
					PageTableEntryType::L012Descriptor(
						&mut *core::ptr::from_mut(self).cast::<L012PageTableDescriptor>(),
					)
				} else {
					PageTableEntryType::L012Block(
						&mut *core::ptr::from_mut(self).cast::<L012PageTableBlock>(),
					)
				}
			}
			3 => {
				if self.0 & 0b10 == 0 {
					PageTableEntryType::Malformed(self)
				} else {
					PageTableEntryType::L3Block(
						&mut *core::ptr::from_mut(self).cast::<L3PageTableBlock>(),
					)
				}
			}
			_ => unreachable!(),
		}
	}
}

/// A single L0/L1/L2 page table descriptor entry.
///
/// # Safety
/// This type is only safe to use as an L0/L1/L2 page table descriptor,
/// and must not be used as an L3 page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct L012PageTableDescriptor(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<L012PageTableDescriptor>(), 8);

impl L012PageTableDescriptor {
	/// Creates a new L0/L1/L2 page table descriptor.
	///
	/// This constructor marks the descriptor as invalid,
	/// but sets its table bit (bit 1).
	///
	/// # Safety
	/// Caller must ensure that the descriptor is not used
	/// as an L3 page table entry.
	#[inline]
	#[must_use]
	pub const unsafe fn new() -> Self {
		Self(0b10)
	}

	/// Resets the L0/L1/L2 page table descriptor to its default state.
	#[inline]
	pub fn reset(&mut self) {
		self.0 = 0b10;
	}
}

impl From<L012PageTableDescriptor> for PageTableEntry {
	#[inline]
	fn from(descriptor: L012PageTableDescriptor) -> Self {
		Self(descriptor.0)
	}
}

/// A single L0/L1/L2 page table block entry.
///
/// # Safety
/// This type is only safe to use as an L0/L1/L2 page table block,
/// and must not be used as an L3 page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct L012PageTableBlock(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<L012PageTableBlock>(), 8);

impl L012PageTableBlock {
	/// Creates a new L0/L1/L2 page table block.
	///
	/// This constructor marks the block as invalid,
	/// and with a clear table bit (bit 1).
	///
	/// # Safety
	/// Caller must ensure that the block is not used
	/// as an L3 page table entry.
	#[inline]
	#[must_use]
	pub const unsafe fn new() -> Self {
		Self(0)
	}

	/// Resets the L0/L1/L2 page table block to its default state.
	#[inline]
	pub fn reset(&mut self) {
		self.0 = 0;
	}
}

impl From<L012PageTableBlock> for PageTableEntry {
	#[inline]
	fn from(block: L012PageTableBlock) -> Self {
		Self(block.0)
	}
}

/// A single L3 page table block entry.
///
/// Note that the bit encoding for a [`L3PageTableBlock`]
/// _is the same†_ as a [`L012PageTableDescriptor`], but the semantics
/// of the address bits is different.
///
/// <sub>†: See [Controlling address translation - Translation table format](https://developer.arm.com/documentation/101811/0103/Controlling-address-translation-Translation-table-format)
/// under final _Note_ block.</sub>
///
/// # Safety
/// This type is only safe to use as an L3 page table block,
/// and must not be used as an L0/L1/L2 page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct L3PageTableBlock(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<L3PageTableBlock>(), 8);

impl L3PageTableBlock {
	/// Creates a new L3 page table block.
	///
	/// This constructor marks the block as invalid,
	/// but sets bit 1 to indicate that the block is valid
	/// (as otherwise the page table would be in a 'reserved'
	/// state).
	///
	/// # Safety
	/// Caller must ensure that the block is used only as an L3
	/// page table entry and **not** as an L0/L1/L2 page table entry.
	#[inline]
	#[must_use]
	pub const unsafe fn new() -> Self {
		Self(0b10)
	}

	/// Resets the L3 page table block to its default state.
	#[inline]
	pub fn reset(&mut self) {
		self.0 = 0b10;
	}
}

impl From<L3PageTableBlock> for PageTableEntry {
	#[inline]
	fn from(block: L3PageTableBlock) -> Self {
		Self(block.0)
	}
}
