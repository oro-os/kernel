//! Extension methods for the MADT table.
#![allow(clippy::inline_always)]

use crate::sys;
use core::mem::ManuallyDrop;
use oro_macro::paste;
use oro_mem::translate::Translator;

/// Indicates that the 8259 PIC is present in the MADT.
const PCAT_COMPAT: u32 = 1;

impl<P: Translator> crate::Madt<P> {
	/// Returns whether or not the 8259 PIC is present in the MADT.
	#[must_use]
	pub fn has_8259(&self) -> bool {
		self.ptr.Flags.read() & PCAT_COMPAT != 0
	}

	/// Returns the physical address of the local APIC.
	#[must_use]
	pub fn lapic_phys(&self) -> u64 {
		u64::from(self.ptr.Address.read())
	}

	/// Returns an iterator over all of the MADT entries.
	#[must_use]
	pub fn entries(&self) -> MadtIterator<'_> {
		MadtIterator::new(
			// SAFETY(qix-): We're guaranteed to be creating a valid slice,
			// SAFETY(qix-): assuming ACPI has reported the correct length.
			unsafe {
				core::slice::from_raw_parts(
					core::ptr::from_ref(self.ptr).cast::<u8>(),
					self.ptr.Header.Length.read() as usize,
				)
			},
		)
	}
}

/// An iterator over the MADT entries.
///
/// Yields `Result`s whereby `Ok` indicates a known
/// MADT entry type and `Err` indicates an unknown entry,
/// providing the raw bytes of the entry (including
/// header and length bytes).
pub struct MadtIterator<'a> {
	/// The current position in the iterator.
	pos:   usize,
	/// The slice of the MADT table.
	slice: &'a [u8],
}

impl<'a> MadtIterator<'a> {
	/// Creates a new iterator over the MADT entries.
	#[must_use]
	pub fn new(slice: &'a [u8]) -> Self {
		Self { pos: 44, slice }
	}
}

impl<'a> Iterator for MadtIterator<'a> {
	type Item = Result<MadtEntry<'a>, &'a [u8]>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.pos >= self.slice.len() {
			return None;
		}

		let un = unsafe { &*(core::ptr::from_ref(&self.slice[self.pos]).cast::<MadtData>()) };
		assert!(unsafe { un.header.Length.read() as usize } <= self.slice.len() - self.pos);

		let pos = self.pos;
		self.pos += unsafe { un.header.Length.read() as usize };

		Some(match un.into() {
			Some(entry) => Ok(entry),
			None => Err(&self.slice[pos..self.pos]),
		})
	}
}

#[expect(clippy::missing_docs_in_private_items)]
macro_rules! madt_entries {
	($($(#[$meta:meta])* $name:tt = $tyid:literal),* $(,)?) => {
		paste! {
			/// Represents an entry in the MADT table.
			#[expect(missing_docs)]
			#[non_exhaustive]
			pub enum MadtEntry<'a> {
				$(
					$(#[$meta])*
					%<title_case:$name>%(&'a sys::acpi_madt_%%$name),
				)*
			}

			impl<'a> core::fmt::Debug for MadtEntry<'a> {
				fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
					match self {
						$(
							MadtEntry::%<title_case:$name>%(_) => write!(f, stringify!(%<title_case:$name>%)),
						)*
					}
				}
			}

			impl<'a> From<&'a MadtData> for Option<MadtEntry<'a>> {
				fn from(data: &'a MadtData) -> Option<MadtEntry<'a>> {
					Some(match unsafe { data.header.Type.read() } {
						$(
							$tyid => MadtEntry::%<title_case:$name>%(unsafe { &data.$name }),
						)*
						_ => return None,
					})
				}
			}

			/// A union of all APIC types. Used by the [`MadtIterator`].
			#[repr(C)]
			union MadtData {
				header: ManuallyDrop<sys::ACPI_SUBTABLE_HEADER>,
				$(
					$(#[$meta])*
					$name: ManuallyDrop<sys::acpi_madt_%%$name>,
				)*
			}
		}
	};
}

madt_entries! {
	local_apic = 0,
	io_apic = 1,
	interrupt_override = 2,
	nmi_source = 3,
	local_apic_nmi = 4,
	local_apic_override = 5,
	io_sapic = 6,
	local_sapic = 7,
	interrupt_source = 8,
	local_x2apic = 9,
	local_x2apic_nmi = 10,
}

/// Extension trait for Local APIC MADT entries.
pub trait LocalApicEx {
	/// Returns the entry.
	fn inner_ref(&self) -> &sys::acpi_madt_local_apic;

	/// Returns the local APIC ID.
	fn id(&self) -> u8 {
		self.inner_ref().Id.read()
	}

	/// Returns whether or not this CPU can be initialized.
	/// If this returns `false`, this entry should be ignored
	/// and the boot routine should not attempt any initialization.
	fn can_init(&self) -> bool {
		self.inner_ref().LapicFlags.read() & 3 != 0
	}
}

impl LocalApicEx for sys::acpi_madt_local_apic {
	#[inline(always)]
	fn inner_ref(&self) -> &sys::acpi_madt_local_apic {
		self
	}
}
