//! Implements the architecture-specific module instance kernel handle.

use oro_mem::mapper::MapError;

use crate::mem::address_space::AddressSpaceHandle;

/// The x86_64 specific module instance kernel handle.
pub struct InstanceHandle {
	/// The mapper handle.
	mapper: AddressSpaceHandle,
}

unsafe impl oro_kernel::arch::InstanceHandle<crate::Arch> for InstanceHandle {
	#[inline]
	fn new(mapper: AddressSpaceHandle) -> Result<Self, MapError> {
		Ok(Self { mapper })
	}

	fn mapper(&self) -> &AddressSpaceHandle {
		&self.mapper
	}
}
