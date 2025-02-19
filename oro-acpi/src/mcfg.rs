//! Extension methods for the MCFG table.
use crate::{AcpiTable, sys};

/// The entry type for the MCFG table.
pub type McfgEntry = sys::acpi_mcfg_allocation;

impl crate::Mcfg {
	/// Returns an iterator over all of the MCFG entries.
	#[must_use]
	pub fn entries(&self) -> McfgIterator {
		McfgIterator::new(
			// SAFETY(qix-): We're guaranteed to be creating a valid slice,
			// SAFETY(qix-): assuming ACPI has reported the correct length.
			unsafe { self.trailing_data() },
		)
	}
}

/// An iterator over the MADT entries.
///
/// Yields `Result`s whereby `Ok` indicates a known
/// MADT entry type and `Err` indicates an unknown entry,
/// providing the raw bytes of the entry (including
/// header and length bytes).
pub struct McfgIterator {
	/// The current position in the iterator.
	pos:   usize,
	/// The slice of the MADT table.
	slice: &'static [u8],
}

impl McfgIterator {
	/// Creates a new iterator over the MADT entries.
	#[must_use]
	pub fn new(slice: &'static [u8]) -> Self {
		Self { pos: 0, slice }
	}
}

impl Iterator for McfgIterator {
	type Item = McfgEntry;

	fn next(&mut self) -> Option<Self::Item> {
		if self.pos >= self.slice.len() {
			return None;
		}

		assert!(core::mem::size_of::<McfgEntry>() <= self.slice.len() - self.pos);
		let un = core::ptr::from_ref::<u8>(&self.slice[self.pos]).cast::<McfgEntry>();

		self.pos += core::mem::size_of::<McfgEntry>();

		// SAFETY(qix-): We're guaranteed to be creating a valid slice,
		// SAFETY(qix-): assuming ACPI has reported the correct length.
		Some(unsafe { un.read_volatile() })
	}
}
