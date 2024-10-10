//! Defines the Oro-specific MAIR entries for the Aarch64 architecture.
#![expect(clippy::inline_always)]

use oro_macro::EnumIterator;

use crate::reg::mair::{MairCacheability, MairDeviceAttribute, MairRegister};

/// The Oro-specific MAIR entries for the Aarch64 architecture.
///
/// Unlike [`crate::reg::mair::MairRegister`], which is a general-purpose MAIR
/// manipulation structure, this enum is specific to Oro and contains
/// the actual entries used by the kernel.
///
/// # Safety
/// The discriminants of this enum are the indices of the MAIR entries.
/// Thus, any modifications must only allow them to be in the range of
/// 0..=7.
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumIterator)]
#[repr(u8)]
pub enum MairEntry {
	/// Memory-Mapped I/O: Device-nGnRnE
	DeviceMMIO   = 0,
	/// General Purpose Normal Memory: Write-Back, Write-Allocate (Cacheable)
	NormalMemory = 1,
	/// Direct-Mapped Physical Pages: Write-Through, No Write-Allocate
	DirectMap    = 2,
	/// IPC Pages: Write-Through, No Write-Allocate
	Ipc          = 3,
}

impl MairEntry {
	/// Returns the MAIR index for this Oro-specific entry type.
	#[inline(always)]
	#[must_use]
	pub const fn index(self) -> u8 {
		unsafe { core::mem::transmute::<Self, u8>(self) }
	}

	/// Builds all of the Oro-specific MAIR entries into a single
	/// [`MairRegister`] value to be loaded into the `MAIR_EL1` register.
	#[inline(always)]
	#[must_use]
	pub fn build_mair() -> MairRegister {
		let mut mair = MairRegister::new();

		for entry in MairEntry::iter_all() {
			match entry {
				MairEntry::DeviceMMIO => {
					*mair[usize::from(entry.index())].device_mut() = MairDeviceAttribute::DnGnRnE;
				}
				MairEntry::NormalMemory => {
					*mair[usize::from(entry.index())].memory_mut() = (
						MairCacheability::WriteBackNonTransientRW,
						MairCacheability::WriteBackNonTransientRW,
					)
						.into();
				}
				MairEntry::DirectMap => {
					*mair[usize::from(entry.index())].memory_mut() = (
						MairCacheability::WriteThroughNonTransient,
						MairCacheability::WriteThroughNonTransient,
					)
						.into();
				}
				MairEntry::Ipc => {
					*mair[usize::from(entry.index())].memory_mut() = (
						MairCacheability::WriteBackTransientRW,
						MairCacheability::WriteBackTransientRW,
					)
						.into();
				}
			}
		}

		mair
	}
}
