#![allow(clippy::unusual_byte_groupings)]

use core::ops::{Index, IndexMut};

/// A page table for the `x86_64` architecture.
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

/// A single page table entry.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PageTableEntry(u64);

static_assertions::const_assert_eq!(::core::mem::size_of::<PageTableEntry>(), 8);

/// Represents a page table entry.
///
/// Each page table entry is a 64-bit value that contains various flags and attributes
/// used for memory management and protection.
///
/// The `PageTableEntry` struct provides methods to manipulate and query the different
/// attributes of a page table entry.
impl PageTableEntry {
	/// Resets the entry to its default state.
	#[inline]
	pub fn reset(&mut self) {
		self.0 = 0;
	}

	/// Creates a new `PageTableEntry` with all flags and attributes set to 0.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Sets the page table entry to the given value.
	#[inline]
	pub fn set(&mut self, value: PageTableEntry) {
		self.0 = value.0;
	}

	/// Checks if the page table entry is present.
	#[inline]
	#[must_use]
	pub fn present(&self) -> bool {
		(self.0 & 1) != 0
	}

	/// Sets the present flag of the page table entry.
	#[inline]
	pub fn set_present(&mut self) {
		self.0 |= 1;
	}

	/// Clears the present flag of the page table entry.
	#[inline]
	pub fn clear_present(&mut self) {
		self.0 &= !1;
	}

	/// Replaces the present flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_present(self) -> Self {
		Self(self.0 | 1)
	}

	/// Checks if the page table entry is writable.
	#[inline]
	#[must_use]
	pub fn writable(&self) -> bool {
		(self.0 & (1 << 1)) != 0
	}

	/// Sets the writable flag of the page table entry.
	#[inline]
	pub fn set_writable(&mut self) {
		self.0 |= 1 << 1;
	}

	/// Clears the writable flag of the page table entry.
	#[inline]
	pub fn clear_writable(&mut self) {
		self.0 &= !(1 << 1);
	}

	/// Replaces the writable flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_writable(self) -> Self {
		Self(self.0 | (1 << 1))
	}

	/// Checks if the page table entry is dirty.
	#[inline]
	#[must_use]
	pub fn dirty(&self) -> bool {
		(self.0 & (1 << 6)) != 0
	}

	/// Sets the dirty flag of the page table entry.
	#[inline]
	pub fn set_dirty(&mut self) {
		self.0 |= 1 << 6;
	}

	/// Clears the dirty flag of the page table entry.
	#[inline]
	pub fn clear_dirty(&mut self) {
		self.0 &= !(1 << 6);
	}

	/// Replaces the dirty flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_dirty(self) -> Self {
		Self(self.0 | (1 << 6))
	}

	/// Checks if the page table entry has been accessed.
	#[inline]
	#[must_use]
	pub fn accessed(&self) -> bool {
		(self.0 & (1 << 5)) != 0
	}

	/// Sets the accessed flag of the page table entry.
	#[inline]
	pub fn set_accessed(&mut self) {
		self.0 |= 1 << 5;
	}

	/// Clears the accessed flag of the page table entry.
	#[inline]
	pub fn clear_accessed(&mut self) {
		self.0 &= !(1 << 5);
	}

	/// Replaces the accessed flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_accessed(self) -> Self {
		Self(self.0 | (1 << 5))
	}

	/// Checks if the page table entry is user-accessible.
	#[inline]
	#[must_use]
	pub fn user(&self) -> bool {
		(self.0 & (1 << 2)) != 0
	}

	/// Sets the user flag of the page table entry.
	#[inline]
	pub fn set_user(&mut self) {
		self.0 |= 1 << 2;
	}

	/// Clears the user flag of the page table entry.
	#[inline]
	pub fn clear_user(&mut self) {
		self.0 &= !(1 << 2);
	}

	/// Replace the user flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_user(self) -> Self {
		Self(self.0 | (1 << 2))
	}

	/// Checks if the page table entry is non-executable.
	#[inline]
	#[must_use]
	pub fn no_exec(&self) -> bool {
		(self.0 & (1 << 63)) != 0
	}

	/// Sets the non-executable flag of the page table entry.
	#[inline]
	pub fn set_no_exec(&mut self) {
		self.0 |= 1 << 63;
	}

	/// Clears the non-executable flag of the page table entry.
	#[inline]
	pub fn clear_no_exec(&mut self) {
		self.0 &= !(1 << 63);
	}

	/// Replaces the non-executable flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_no_exec(self) -> Self {
		Self(self.0 | (1 << 63))
	}

	/// Checks if the page table entry is global.
	#[inline]
	#[must_use]
	pub fn global(&self) -> bool {
		(self.0 & (1 << 8)) != 0
	}

	/// Sets the global flag of the page table entry.
	#[inline]
	pub fn set_global(&mut self) {
		self.0 |= 1 << 8;
	}

	/// Clears the global flag of the page table entry.
	#[inline]
	pub fn clear_global(&mut self) {
		self.0 &= !(1 << 8);
	}

	/// Replaces the global flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_global(self) -> Self {
		Self(self.0 | (1 << 8))
	}

	/// Checks if the page table entry has write-through caching enabled.
	#[inline]
	#[must_use]
	pub fn write_through(&self) -> bool {
		(self.0 & (1 << 3)) != 0
	}

	/// Sets the write-through flag of the page table entry.
	#[inline]
	pub fn set_write_through(&mut self) {
		self.0 |= 1 << 3;
	}

	/// Clears the write-through flag of the page table entry.
	#[inline]
	pub fn clear_write_through(&mut self) {
		self.0 &= !(1 << 3);
	}

	/// Replaces the write-through flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_write_through(self) -> Self {
		Self(self.0 | (1 << 3))
	}

	/// Checks if the page table entry has caching disabled.
	#[inline]
	#[must_use]
	pub fn cache_disable(&self) -> bool {
		(self.0 & (1 << 4)) != 0
	}

	/// Sets the cache-disable flag of the page table entry.
	#[inline]
	pub fn set_cache_disable(&mut self) {
		self.0 |= 1 << 4;
	}

	/// Clears the cache-disable flag of the page table entry.
	#[inline]
	pub fn clear_cache_disable(&mut self) {
		self.0 &= !(1 << 4);
	}

	/// Replaces the cache-disable flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_cache_disable(self) -> Self {
		Self(self.0 | (1 << 4))
	}

	/// Checks if the page table entry is supervisor-only.
	#[inline]
	#[must_use]
	pub fn supervisor(&self) -> bool {
		(self.0 & (1 << 2)) != 0
	}

	/// Sets the supervisor flag of the page table entry.
	#[inline]
	pub fn set_supervisor(&mut self) {
		self.0 &= !(1 << 2);
	}

	/// Clears the supervisor flag of the page table entry.
	#[inline]
	pub fn clear_supervisor(&mut self) {
		self.0 |= 1 << 2;
	}

	/// Replaces the supervisor flag, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_supervisor(self) -> Self {
		Self(self.0 & !(1 << 2))
	}

	/// Gets a manipulator for the "available" fields of the page table entry.
	#[inline]
	#[must_use]
	pub fn available(&mut self) -> AvailableFields {
		AvailableFields(&mut self.0)
	}

	/// Replaces the available bits of the page table entry with the given 10-bit value,
	/// returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_available(self, value: u16) -> Self {
		Self(
			(self.0 & !(0b111 << 9) & !(0b111_1111 << 52))
				| (((value as u64) & 0b111) << 9)
				| (((value as u64) & 0b11_1111_1000) << 49),
		)
	}

	/// Checks if the page is a huge page.
	///
	/// # Safety
	/// Must only be called on a PTPD or a PD entry.
	#[inline]
	#[must_use]
	pub unsafe fn huge(&self) -> bool {
		(self.0 & (1 << 7)) != 0
	}

	/// Sets the huge flag of the page table entry.
	///
	/// # Safety
	/// Must only be called on a PTPD or a PD entry.
	#[inline]
	pub unsafe fn set_huge(&mut self) {
		self.0 |= 1 << 7;
	}

	/// Clears the huge flag of the page table entry.
	///
	/// # Safety
	/// Must only be called on a PTPD or a PD entry.
	#[inline]
	pub unsafe fn clear_huge(&mut self) {
		self.0 &= !(1 << 7);
	}

	/// Replaces the huge flag, returning a new `PageTableEntry`.
	///
	/// # Safety
	/// Must only be called on a PTPD or a PD entry.
	#[inline]
	#[must_use]
	pub unsafe fn with_huge(self) -> Self {
		Self(self.0 | (1 << 7))
	}

	/// Sets the physical address of the page table entry.
	#[inline]
	pub fn set_address(&mut self, address: u64) {
		unsafe { self.set_address_unchecked(address & 0xFFF_0000000000_FFF) }
	}

	/// Sets the physical address of the page table entry, without truncating bits.
	/// This is only slightly faster than `set_address`, and should only be used if
	/// the address is known to be properly aligned and truncated.
	///
	/// # Safety
	/// The address must be properly aligned and truncated (i.e. `(address & 0xFFF_0000000000_FFF) == 0`).
	#[inline]
	pub unsafe fn set_address_unchecked(&mut self, address: u64) {
		self.0 = (self.0 & 0xFFF_0000000000_FFF) | address;
	}

	/// Replaces the physical address of the page table, returning a new `PageTableEntry`.
	#[inline]
	#[must_use]
	pub const fn with_address(self, address: u64) -> Self {
		Self((self.0 & 0xFFF_0000000000_FFF) | (address & 0x000_FFFFFFFFFF_000))
	}

	/// Gets the physical address of the page table entry.
	#[inline]
	#[must_use]
	pub fn address(&self) -> u64 {
		self.0 & 0x000_FFFFFFFFFF_000
	}
}

impl From<u64> for PageTableEntry {
	#[inline]
	#[must_use]
	fn from(value: u64) -> Self {
		Self(value)
	}
}

impl From<PageTableEntry> for u64 {
	#[inline]
	#[must_use]
	fn from(entry: PageTableEntry) -> Self {
		entry.0
	}
}

/// A struct for manipulating the available fields of a page table entry.
pub struct AvailableFields<'a>(&'a mut u64);

impl<'a> AvailableFields<'a> {
	/// Gets the first block of available bits as a u8.
	#[inline]
	#[must_use]
	pub fn first(&self) -> u8 {
		(((*self.0) >> 9) & 0b111) as u8
	}

	/// Sets the first block of available bits as a u8.
	#[inline]
	pub fn set_first(&mut self, value: u8) {
		*self.0 = (*self.0 & !(0b111 << 9)) | (u64::from(value) << 9);
	}

	/// Gets the second block of available bits as a u8.
	#[inline]
	#[must_use]
	pub fn second(&self) -> u8 {
		(((*self.0) >> 52) & 0b111_1111) as u8
	}

	/// Sets the second block of available bits as a u8.
	#[inline]
	pub fn set_second(&mut self, value: u8) {
		*self.0 = (*self.0 & !(0b111_1111 << 52)) | (u64::from(value) << 52);
	}

	/// Gets both the first and second blocks of available fields encoded as a 10-bit u16.
	#[inline]
	#[must_use]
	pub fn as_u16(&self) -> u16 {
		u16::from(self.first()) | (u16::from(self.second()) << 3)
	}

	/// Sets both the first and second blocks of available fields encoded as a 10-bit u16.
	#[inline]
	pub fn set_u16(&mut self, value: u16) {
		*self.0 = (*self.0) & !((0b111 << 9) | (0b111_1111 << 52))
			| ((u64::from(value) & 0b111) << 9)
			| ((u64::from(value) & 0b11_1111_1000) << 49);
	}

	/// Sets a single bit in the available fields. Bit must be in the range `0..=9`.
	#[inline]
	pub fn set(&mut self, bit: usize) {
		debug_assert!(bit < 10, "bit out of bounds (max 9)");

		let bit = bit + 9 + (40 * usize::from(bit > 2));
		*self.0 |= 1 << bit;
	}

	/// Clears a single bit in the available fields. Bit must be in the range `0..=9`.
	#[inline]
	pub fn clear(&mut self, bit: usize) {
		debug_assert!(bit < 10, "bit out of bounds (max 9)");

		let bit = bit + 9 + (40 * usize::from(bit > 2));
		*self.0 &= !(1 << bit);
	}

	/// Gets a single bit in the available fields. Bit must be in the range `0..=9`.
	#[inline]
	#[must_use]
	pub fn get(&self, bit: usize) -> bool {
		debug_assert!(bit < 10, "bit out of bounds (max 9)");

		let bit = bit + 9 + (40 * usize::from(bit > 2));
		((*self.0) & (1 << bit)) != 0
	}
}
