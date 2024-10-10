//! Oro support for the Advanced Configuration and Power Interface (ACPI)
//! specification.
#![cfg_attr(not(test), no_std)]
// SAFETY(qix-): This is approved, just moving slowly.
// SAFETY(qix-): It's also not critical to the operation of the crate.
// https://github.com/rust-lang/rust/issues/48214
#![feature(trivial_bounds)]

use core::ptr::from_ref;

pub use ::oro_acpica_sys as sys;
use oro_macro::assert;
use oro_mem::phys::{Phys, PhysAddr};

pub mod madt;

/// RSDP structure.
pub struct Rsdp {
	/// The pointer to the RSDP structure.
	/// SAFETY(qix-): Revision must be checked before accessing bytes beyond the first 20.
	ptr: &'static sys::acpi_table_rsdp,
}

impl Rsdp {
	/// Gets and validates the RSDP structure from
	/// the given physical address.
	///
	/// Returns `None` if the RSDP structure is invalid / not-aligned.
	///
	/// # Safety
	/// Caller must ensure that the physical address is valid and points
	/// to a valid RSDP structure. This typically means making the assumption
	/// the bootloader has done so, but we still must mark this as unsafe.
	#[must_use]
	pub unsafe fn get(physical_address: u64) -> Option<Self> {
		let ptr =
			Phys::from_address_unchecked(physical_address).as_ref::<sys::acpi_table_rsdp>()?;

		if ptr.Signature != *core::ptr::from_ref(sys::ACPI_SIG_RSDP).cast::<[i8; 8]>() {
			return None;
		}

		let mut checksum: u8 = 0;
		for i in 0..20 {
			checksum = checksum.wrapping_add(from_ref(ptr).cast::<u8>().add(i).read());
		}

		if checksum != 0 {
			return None;
		}

		if ptr.Revision.read() > 0 {
			// Perform an extended checksum
			// SAFETY(qix-): The length field is only valid for revisions > 0.
			let mut checksum: u8 = 0;
			for i in 0..(ptr.Length.read() as usize) {
				checksum = checksum.wrapping_add(from_ref(ptr).cast::<u8>().add(i).read());
			}
			if checksum != 0 {
				return None;
			}
		}

		Some(Self { ptr })
	}

	/// Gets the revision.
	#[must_use]
	pub fn revision(&self) -> u8 {
		self.ptr.Revision.read()
	}

	/// Gets the (X)SDT.
	///
	/// Returns `None` if the validation of the table fails.
	#[must_use]
	pub fn sdt(&self) -> Option<RootSdt> {
		if self.revision() == 0 {
			// SAFETY(qix-): We've made sure we're casting to the right type.
			Some(RootSdt::Rsdt(unsafe {
				Rsdt::new(u64::from(self.ptr.RsdtPhysicalAddress.read()))?
			}))
		} else {
			// SAFETY(qix-): We've made sure we're casting to the right type.
			Some(RootSdt::Xsdt(unsafe {
				Xsdt::new(self.ptr.XsdtPhysicalAddress.read())?
			}))
		}
	}
}

// SAFETY(qix-): Uses unstable feature `trivial_bounds`.
#[expect(trivial_bounds)]
impl core::fmt::Debug for Rsdp
where
	sys::acpi_table_rsdp: core::fmt::Debug,
{
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		self.ptr.fmt(f)
	}
}

/// Either the revision 0 SDT or the the eXtended SDT.
pub enum RootSdt {
	/// The revision 0 SDT.
	Rsdt(Rsdt),
	/// The eXtended SDT.
	Xsdt(Xsdt),
}

/// X/RSDT table search trait, allowing for searching for a table by signature.
pub trait RootSdtSearch<const PTR_SIZE: usize>: AcpiTable
where
	[u8; PTR_SIZE]: FromLe64<PTR_SIZE>,
{
	/// Searches for a table by signature.
	///
	/// Returns `None` if the table is not found or is not valid.
	fn find<T: AcpiTable>(&self) -> Option<T> {
		// SAFETY(qix-): We know that the data is valid since we've validated the table.
		unsafe {
			<Self as AcpiTable>::data(self)
				.as_window_slice::<{ PTR_SIZE }>()
				.iter()
				.find_map(|&chunk| {
					let phys = chunk.from_le_64();
					let sig = Phys::from_address_unchecked(phys).as_ref_unchecked::<[i8; 4]>();
					if sig == T::SIGNATURE {
						// SAFETY(qix-): We've ensured that we're really iterating over physical addresses.
						Some(T::new(phys))
					} else {
						None
					}
				})?
		}
	}
}

impl RootSdtSearch<4> for Rsdt {}
impl RootSdtSearch<8> for Xsdt {}

impl RootSdt {
	/// Searches for a table by signature. Automatically
	/// selects the correct table to search based on the
	/// revision of the RSDP.
	#[must_use]
	pub fn find<T: AcpiTable>(&self) -> Option<T> {
		match self {
			Self::Rsdt(rsdt) => rsdt.find(),
			Self::Xsdt(xsdt) => xsdt.find(),
		}
	}
}

/// Base ACPI table trait. All ACPI tables (except for the `RSDP`)
/// implement this trait.
pub trait AcpiTable: Sized {
	/// The signature of the ACPI table.
	const SIGNATURE: &'static [i8; 4];

	/// The underlying system table type from [`oro_acpica_sys`].
	type SysTable: Sized + 'static;

	/// Creates a new instance of the ACPI table
	/// from the given physical address.
	///
	/// Returns `None` if the ACPI table is invalid.
	///
	/// # Safety
	/// Caller must ensure the physical address is readable.
	#[must_use]
	unsafe fn new(physical_address: u64) -> Option<Self> {
		let ptr = Phys::from_address_unchecked(physical_address).as_ref::<Self::SysTable>()?;
		let header = Self::header_ref(ptr);

		if &header.Signature != Self::SIGNATURE {
			return None;
		}

		let mut checksum = 0_u8;
		for i in 0..header.Length.read() {
			assert::fits_within::<u32, usize>();
			checksum = checksum.wrapping_add(from_ref(ptr).cast::<u8>().add(i as usize).read());
		}

		if checksum != 0 {
			return None;
		}

		Some(Self::new_unchecked(ptr))
	}

	/// Creates a new instance of the ACPI table
	/// from the given physical address.
	///
	/// Does NOT perform any validation. **Do not use this method.
	/// Use [`Self::new`] instead.**
	///
	/// # Safety
	/// Caller must ensure the physical address is readable and that
	/// the ACPI table is valid.
	unsafe fn new_unchecked(ptr: &'static Self::SysTable) -> Self;

	/// Returns a reference to the header of a given ref.
	///
	/// # Safety
	/// Caller must treat any and all multibyte fields fetched
	/// from within this header as little endian.
	unsafe fn header_ref(sys_table: &Self::SysTable) -> &sys::acpi_table_header;

	/// Returns a reference to this table's header.
	///
	/// # Safety
	/// Caller must treat any and all multibyte fields fetched
	/// from within this header as little endian.
	unsafe fn header(&self) -> &sys::acpi_table_header;

	/// Returns a slice of the table's data (after the header).
	///
	/// # Safety
	/// Caller must treat any and all multibyte fields fetched
	/// from within this data as little endian.
	unsafe fn data(&self) -> &[u8] {
		// SAFETY(qix-): We can assume that the data is valid since
		// SAFETY(qix-): this object only exists if it was validated.
		// SAFETY(qix-): If it is not valid, it's a bug in the ACPI table implementation.
		unsafe {
			let header = self.header();
			// SAFETY(qix-): We perform a static assertion to make sure the convertion
			// SAFETY(qix-): from u32 to usize won't truncate.
			assert::fits_within::<u32, usize>();
			let len =
				header.Length.read() as usize - core::mem::size_of::<sys::acpi_table_header>();
			let data_base = core::ptr::from_ref(header).add(1).cast::<u8>();
			return core::slice::from_raw_parts(data_base, len);
		}
	}

	/// Returns the internal system table type.
	///
	/// # Safety
	/// Caller must access all multi-byte fields as little endian.
	unsafe fn inner_ref(&self) -> &Self::SysTable {
		// SAFETY(qix-): The header reference always marks the start of the table.
		unsafe { &*::core::ptr::from_ref(self.header()).cast::<Self::SysTable>() }
	}
}

/// Generation macro that creates wrapper structs around the lower level
/// [`oro_acpica_sys`] table types and implements the higher level wrapper
/// and marker traits for them.
///
/// This macro itself is invoked by [`oro_acpica_sys::acpi_tablegen`], which
/// uses a statically generated list of ACPI tables discovered from the
/// Intel ACPICA library.
macro_rules! impl_tables {
	// In the case of Rsdp, its signature size is 8 and has an overall
	// different structure, so we exclude it.
	(@inner Rsdp => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),*)) => {};
	// Some types that don't have the conventional header
	(@inner Facs => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),*)) => {};
	(@inner S3Pt => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),*)) => {};
	(@inner Cdat => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),*)) => {};

	(@inner $ident:ident => ($systbl_ident:ty, $sig_ident:path, ( & [ u8; 5 ] ), $(#[doc = $doc:literal]),*)) => {
		#[allow(missing_docs, rustdoc::bare_urls)]
		$(#[doc = $doc])*
		pub struct $ident {
			ptr: &'static $systbl_ident,
		}

		impl AcpiTable for $ident {
			// SAFETY(qix-): We can guarantee that the signature is the right size.
			const SIGNATURE: &'static [i8; 4] = unsafe {
				&*from_ref($sig_ident).cast::<[i8; 4]>()
			};

			type SysTable = $systbl_ident;

			unsafe fn new_unchecked(ptr: &'static Self::SysTable) -> Self {
				Self { ptr }
			}

			unsafe fn header_ref(sys_table: &Self::SysTable) -> &sys::acpi_table_header {
				&sys_table.Header
			}

			unsafe fn header(&self) -> &sys::acpi_table_header {
				&self.ptr.Header
			}
		}

		// SAFETY(qix-): Uses unstable feature `trivial_bounds`.
		#[expect(trivial_bounds)]
		impl core::fmt::Debug for $ident where $systbl_ident: core::fmt::Debug {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				self.ptr.fmt(f)
			}
		}
	};

	(@inner $ident:ident => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),*)) => {
		compile_error!(concat!("ACPI table has unsupported signature size: ", stringify!(($ident, $systbl_ident, $sig_ident, $($sig_type)*))));
	};

	($($ident:ident => ($systbl_ident:ty, $sig_ident:path, ( $($sig_type:tt)* ), $(#[doc = $doc:literal]),* $(,)?)),* $(,)?) => {
		$(impl_tables!(@inner $ident => ($systbl_ident, $sig_ident, ($($sig_type)*), $(#[doc = $doc]),*));)*
	};
}

sys::acpi_tablegen!(impl_tables);

/// Helper trait that casts a slice of bytes as a windowed slice
/// of bytes, with any trailing bytes (less than the window size)
/// not included.
trait AsWindowSlice<'a, Inner: Sized + 'a>: 'a + AsRef<[Inner]> {
	/// Casts the slice as a windowed slice.
	///
	/// N cannot be zero.
	fn as_window_slice<const N: usize>(&'a self) -> &'a [[Inner; N]] {
		let this = self.as_ref();
		let len = this.len();
		let total_elements = len - (len % N);
		// No real way around it. If you can think of one, a PR would be appreciated.
		#[allow(clippy::integer_division)]
		let total_windows = total_elements / N;
		let base = this.as_ptr().cast::<[Inner; N]>();
		// SAFETY(qix-): We know that the slice is valid and that the window size is valid.
		unsafe { ::core::slice::from_raw_parts(base, total_windows) }
	}
}

impl<'a, T: Sized> AsWindowSlice<'a, T> for &'a [T] {}

/// Converts a byte array in little-endian to a `u64`
/// in host endianness.
pub trait FromLe64<const ORIG: usize> {
	/// Treat `self` as an array of little-endian bytes
	/// and convert them to a `u64` in host order.
	// TODO(qix-): the numbers common lib will probably have better names for this.
	// TODO(qix-): When this is copied over, remove the `#[allow(clippy::wrong_self_convention)]`
	// TODO(qix-): and let's find a better name.
	#[expect(clippy::wrong_self_convention)]
	fn from_le_64(self) -> u64;
}

impl<T: Into<[u8; 4]>> FromLe64<4> for T {
	fn from_le_64(self) -> u64 {
		u64::from(u32::from_le_bytes(self.into()))
	}
}

impl<T: Into<[u8; 8]>> FromLe64<8> for T {
	fn from_le_64(self) -> u64 {
		u64::from_le_bytes(self.into())
	}
}

impl<T: Into<[u8; 2]>> FromLe64<2> for T {
	fn from_le_64(self) -> u64 {
		u64::from(u16::from_le_bytes(self.into()))
	}
}

impl<T: Into<[u8; 1]>> FromLe64<1> for T {
	fn from_le_64(self) -> u64 {
		u64::from(self.into()[0])
	}
}

impl<T: Into<[u8; 0]>> FromLe64<0> for T {
	fn from_le_64(self) -> u64 {
		0
	}
}
