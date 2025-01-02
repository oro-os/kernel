//! Implements the architecture-specific module instance kernel handle.

use oro_mem::mapper::MapError;

use crate::mem::address_space::Ttbr0Handle;

/// The AArch64 specific module instance kernel handle.
pub struct InstanceHandle {
	/// The mapper handle.
	mapper: Ttbr0Handle,
}

unsafe impl oro_kernel::arch::InstanceHandle<crate::Arch> for InstanceHandle {
	#[inline]
	fn new(mapper: Ttbr0Handle) -> Result<Self, MapError> {
		Ok(Self { mapper })
	}

	fn mapper(&self) -> &Ttbr0Handle {
		&self.mapper
	}
}
