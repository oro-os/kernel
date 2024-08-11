//! Implements ACPI-related functionality.

use ::acpi::handler::{AcpiHandler, PhysicalMapping};
use ::oro_common::mem::translate::PhysicalAddressTranslator;
use core::ptr::NonNull;

/// A newtype that implements an [`AcpiHandler`] for
/// any [`PhysicalAddressTranslator`].
#[derive(Clone)]
pub struct TranslatorAcpiHandler<P: PhysicalAddressTranslator + 'static> {
	/// The physical address translator to use.
	translator: P,
}

impl<P: PhysicalAddressTranslator + 'static> TranslatorAcpiHandler<P> {
	/// Creates a new [`PhysicalAddressTranslator`]-based ACPI handler.
	#[must_use]
	pub fn new(translator: P) -> Self {
		Self { translator }
	}
}

impl<P: PhysicalAddressTranslator> AcpiHandler for TranslatorAcpiHandler<P> {
	unsafe fn map_physical_region<T>(
		&self,
		physical_address: usize,
		size: usize,
	) -> PhysicalMapping<Self, T> {
		() = ::oro_common::util::assertions::assert_fits_within::<usize, u64>();

		PhysicalMapping::new(
			physical_address,
			NonNull::new_unchecked(
				self.translator.to_virtual_addr(physical_address as u64) as *mut T
			),
			size,
			size,
			self.clone(),
		)
	}

	fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
		// No-op.
	}
}

// NOTE(qix-): Dumb debug function such that the ACPI tables can themselves
// NOTE(qix-): be debug formatted. Simply prints `TranslatorAcpiHandler`.
impl<P: PhysicalAddressTranslator> ::core::fmt::Debug for TranslatorAcpiHandler<P> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("TranslatorAcpiHandler")
			.finish_non_exhaustive()
	}
}
