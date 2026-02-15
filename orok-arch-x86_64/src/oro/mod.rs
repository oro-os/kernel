//! Oro-specific x86_64 architecture facilities and types, built on top of the
//! architecture-agnostic traits and types defined in `orok-arch-base`.

mod page_size;
mod unsafe_addr;

/// Implements the x86_64 architecture.
#[non_exhaustive]
pub struct Arch;

impl orok_arch_base::Arch for Arch {
	type PageSize = page_size::PageSize;
	type UnsafePhys = unsafe_addr::UnsafePhys;
	type UnsafeVirt = unsafe_addr::UnsafeVirt;

	unsafe fn init() {
		// SAFETY: Safety considerations offloaded to caller.
		unsafe {
			crate::arch::refresh_globals();
		}
	}
}
