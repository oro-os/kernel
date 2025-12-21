//! MAIR register implementation for Aarch64.
//!
//! # Notes
//! Some notes on the implementation of the aarch64 MAIR register
//! types:
//!
//! - This implementation assumes its working only with the `MAIR_EL1`
//!   register. Other exception level MAIR registers are not explicitly
//!   supported.

#![expect(clippy::inline_always, clippy::module_name_repetitions)]

use core::{
	fmt,
	ptr::{from_mut, from_ref},
};

/// An accessor around a MAIR register value.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct MairRegister(u64);

impl MairRegister {
	/// Creates a blank MAIR register value.
	#[inline(always)]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Gets a copy of the memory attributes
	/// for a specific entry in the MAIR register.
	///
	/// # Safety
	/// The `index` must be in the range `0..=7`.
	#[must_use]
	#[inline(always)]
	pub unsafe fn get(self, index: usize) -> MairAttributes {
		debug_assert!(index < 8, "index must be 0..=7");
		let shift = index * 8;
		#[expect(clippy::cast_possible_truncation)]
		MairAttributes((self.0 >> shift) as u8)
	}

	/// Sets the memory attributes for a specific
	/// entry in the MAIR register.
	///
	/// # Safety
	/// The `index` must be in the range `0..=7`.
	#[inline(always)]
	pub unsafe fn set(&mut self, index: usize, attrs: MairAttributes) {
		debug_assert!(index < 8, "index must be 0..=7");
		let shift = index * 8;
		let mask = 0xFF << shift;
		self.0 = (self.0 & !mask) | ((u64::from(attrs.0)) << shift);
	}

	/// Replaces the memory attributes for a specific
	/// entry in the MAIR register.
	///
	/// # Safety
	/// The `index` must be in the range `0..=7`.
	/// **Values higher than 7 will have their bits masked.**
	#[must_use]
	#[inline(always)]
	pub const unsafe fn with(self, index: usize, attrs: MairAttributes) -> Self {
		let shift = (index & 0b111) * 8;
		let mask = 0xFF << shift;
		Self((self.0 & !mask) | ((attrs.0 as u64) << shift))
	}

	/// Gets a specific entry in the MAIR register
	/// as an immutable reference.
	///
	/// # Safety
	/// The `index` must be in the range `0..=7`.
	#[must_use]
	#[inline(always)]
	pub unsafe fn get_ref(&self, index: usize) -> &MairAttributes {
		debug_assert!(index < 8, "index must be 0..=7");
		unsafe {
			&*from_ref(&((&*from_ref::<u64>(&self.0).cast::<[u8; 8]>())[7 - index]))
				.cast::<MairAttributes>()
		}
	}

	/// Gets a specific entry in the MAIR register
	/// as a mutable reference.
	///
	/// # Safety
	/// The `index` must be in the range `0..=7`.
	#[must_use]
	#[inline(always)]
	pub unsafe fn get_mut(&mut self, index: usize) -> &mut MairAttributes {
		debug_assert!(index < 8, "index must be 0..=7");
		unsafe {
			&mut *from_mut(&mut ((&mut *from_mut::<u64>(&mut self.0).cast::<[u8; 8]>())[7 - index]))
				.cast::<MairAttributes>()
		}
	}
}

impl core::ops::IndexMut<usize> for MairRegister {
	#[inline(always)]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		unsafe { self.get_mut(index) }
	}
}

impl core::ops::Index<usize> for MairRegister {
	type Output = MairAttributes;

	#[inline(always)]
	fn index(&self, index: usize) -> &Self::Output {
		unsafe { self.get_ref(index) }
	}
}

impl fmt::Debug for MairRegister {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_list()
			.entries((0..8).map(|i| unsafe { self.get(i) }))
			.finish()
	}
}

// NOTE(qix-): I don't really want to encourage anyone convert back from a raw u64
// NOTE(qix-): to a `MairRegister` value as that's probably not very safe.
#[expect(clippy::from_over_into)]
impl Into<u64> for MairRegister {
	#[inline(always)]
	fn into(self) -> u64 {
		self.0
	}
}

/// Memory attributes for a specific MAIR attribute.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MairAttributes(u8);

impl MairAttributes {
	/// Creates a new set of memory attributes.
	#[inline(always)]
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Indicates the memory is device memory;
	/// returns a device memory attribute mutator.
	///
	/// **Resets the current attributes if they're not already
	/// set to device memory.**
	#[inline(always)]
	#[must_use]
	pub fn device_mut(&mut self) -> &mut MairDeviceAttribute {
		self.0 = u8::from((self.0 & 0b1111_0000) != 0) * (self.0 & 0b0000_1111);
		unsafe { &mut *from_mut(&mut self.0).cast::<MairDeviceAttribute>() }
	}

	/// Indicates the memory is normal memory;
	/// returns a normal memory attribute mutator.
	///
	/// **Does _NOT_ reset bits, even if the highest
	/// four bits are zero (indicating device memory).**
	#[inline(always)]
	#[must_use]
	pub fn memory_mut(&mut self) -> &mut MairMemoryAttributes {
		unsafe { &mut *from_mut(&mut self.0).cast::<MairMemoryAttributes>() }
	}

	/// Gets the type of memory these attributes represent.
	#[inline(always)]
	#[must_use]
	pub fn ty(self) -> AttributesType {
		if self.0 & 0b1111_0000 == 0 {
			// SAFETY: We can guarantee the value is valid here as the transmuted
			// SAFETY: value is always in the range of a valid `MairDeviceAttribute`.
			AttributesType::Device(unsafe {
				core::mem::transmute::<u8, MairDeviceAttribute>(self.0)
			})
		} else {
			// SAFETY: We can guarantee the value is valid here as the transmuted
			// SAFETY: value is always in the range of a valid `MairMemoryAttributes`.
			AttributesType::Memory(unsafe {
				core::mem::transmute::<u8, MairMemoryAttributes>(self.0)
			})
		}
	}
}

impl fmt::Debug for MairAttributes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.ty() {
			AttributesType::Device(attr) => attr.fmt(f),
			AttributesType::Memory(attr) => attr.fmt(f),
		}
	}
}

impl Default for MairAttributes {
	#[inline(always)]
	fn default() -> Self {
		Self::new()
	}
}

/// The type of memory a specific set of [`MairAttributes`] represents.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AttributesType {
	/// The attribute is a device attribute (as opposed to a memory attribute).
	Device(MairDeviceAttribute),
	/// The attribute is a memory attribute (as opposed to a device attribute).
	Memory(MairMemoryAttributes),
}

impl fmt::Debug for AttributesType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Device(attr) => attr.fmt(f),
			Self::Memory(attr) => attr.fmt(f),
		}
	}
}

/// Device memory attributes for a specific MAIR attribute.
///
/// See section E2.8.2 of the ARMv8-A Architecture Reference Manual
/// (ARM DDI 0487A.a) for more information.
///
/// This enum is non-exhaustive due to certain bit sequences
/// representing "UNPREDICTABLE" values according to the ARM manual,
/// which Oro does not support. Since this enum is transmuted from
/// the lower four bits of the attribute value, this may yield
/// "UNPREDICTABLE" values if the attribute is not correctly
/// initialized.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum MairDeviceAttribute {
	/// Device non-Gathering, non-Reordering, No Early write acknowledgement.
	DnGnRnE = 0b0000,
	/// Device non-Gathering, non-Reordering, Early write acknowledgement.
	DnGnRE  = 0b0100,
	/// Device non-Gathering, Reordering, Early Write Acknowledgement.
	DnGRE   = 0b1000,
	/// Device Gathering, Reordering, Early Write Acknowledgement.
	DGRE    = 0b1100,
}

impl fmt::Debug for MairDeviceAttribute {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::DnGnRnE => write!(f, "Device(nGnRnE)"),
			Self::DnGnRE => write!(f, "Device(nGnRE)"),
			Self::DnGRE => write!(f, "Device(nGRE)"),
			Self::DGRE => write!(f, "Device(GRE)"),
		}
	}
}

/// Normal memory attributes for a specific MAIR attribute.
///
/// See section E2.8.1 of the ARMv8-A Architecture Reference Manual
/// (ARM DDI 0487A.a) for more information.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MairMemoryAttributes(u8);

impl fmt::Debug for MairMemoryAttributes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let (outer, inner) = (*self).into();
		write!(f, "Memory(outer={outer:?}, inner={inner:?})")
	}
}

/// Outer cacheability attribute shift for a normal memory
/// MAIR attribute. Used with [`MairCacheability`].
pub const OUTER: u8 = 4;
/// Inner cacheability attribute shift for a normal memory
/// MAIR attribute. Used with [`MairCacheability`].
pub const INNER: u8 = 0;

/// Cacheability setting for a specific MAIR attribute
/// under normal memory mode.
///
/// Note that this enum is non-exhaustive due to certain
/// bit sequences representing "UNPREDICTABLE" values according
/// to the ARM manual, which Oro does not support.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MairCacheability<const SHIFT: u8> {
	/// Write-through transient
	/// with read allocate policy
	WriteThroughTransientR     = 0b0010,
	/// Write-through transient
	/// with write allocate policy
	WriteThroughTransientW     = 0b0001,
	/// Write-through transient
	/// with both read and write allocate policy
	WriteThroughTransientRW    = 0b0011,
	/// Non-Cacheable
	NonCacheable               = 0b0100,
	/// Outer Write-back transient
	/// with read allocate policy
	WriteBackTransientR        = 0b0110,
	/// Outer Write-back transient
	/// with write allocate policy
	WriteBackTransientW        = 0b0101,
	/// Outer Write-back transient
	/// with both read and write allocate policy
	WriteBackTransientRW       = 0b0111,
	/// Write-through non-transient
	/// with neither read nor write allocate policy
	WriteThroughNonTransient   = 0b1000,
	/// Write-through non-transient
	/// with read allocate policy
	WriteThroughNonTransientR  = 0b1010,
	/// Write-through non-transient
	/// with write allocate policy
	WriteThroughNonTransientW  = 0b1001,
	/// Write-through non-transient
	/// with both read and write allocate policy
	WriteThroughNonTransientRW = 0b1011,
	/// Write-back non-transient
	/// with neither read nor write allocate policy
	WriteBackNonTransient      = 0b1100,
	/// Write-back non-transient
	/// with read allocate policy
	WriteBackNonTransientR     = 0b1110,
	/// Write-back non-transient
	/// with write allocate policy
	WriteBackNonTransientW     = 0b1101,
	/// Write-back non-transient
	/// with both read and write allocate policy
	WriteBackNonTransientRW    = 0b1111,
}

impl From<(MairCacheability<OUTER>, MairCacheability<INNER>)> for MairMemoryAttributes {
	#[inline(always)]
	fn from((outer, inner): (MairCacheability<OUTER>, MairCacheability<INNER>)) -> Self {
		MairMemoryAttributes(unsafe {
			((core::mem::transmute::<MairCacheability<OUTER>, u8>(outer) & 0b1111) << OUTER)
				| ((core::mem::transmute::<MairCacheability<INNER>, u8>(inner) & 0b1111) << INNER)
		})
	}
}

impl From<MairMemoryAttributes> for (MairCacheability<OUTER>, MairCacheability<INNER>) {
	#[inline(always)]
	fn from(attrs: MairMemoryAttributes) -> Self {
		(
			// SAFEY: We always know that this results in a valid value.
			unsafe {
				core::mem::transmute::<u8, MairCacheability<OUTER>>((attrs.0 >> OUTER) & 0b1111)
			},
			// SAFEY: We always know that this results in a valid value.
			unsafe {
				core::mem::transmute::<u8, MairCacheability<INNER>>((attrs.0 >> INNER) & 0b1111)
			},
		)
	}
}
