//! Extension methods for the MADT table.

use oro_common::mem::translate::PhysicalAddressTranslator;

/// Indicates that the 8259 PIC is present in the MADT.
const PCAT_COMPAT: u32 = 1;

impl<P: PhysicalAddressTranslator> crate::Madt<P> {
	/// Returns whether or not the 8259 PIC is present in the MADT.
	#[must_use]
	pub fn has_8259(&self) -> bool {
		self.ptr.Flags & PCAT_COMPAT != 0
	}
}
