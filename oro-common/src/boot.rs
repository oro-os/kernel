//! Boot protocol types and serializer implementation.
//! Provides boot-time information to the kernel from the boot
//! stage configuration.
//!
//! Note that no core-specific information is provided here, as
//! that is handled by passing information to the kernel via
//! architecture-specific transfer stubs.

#[repr(C, align(4096))]
pub struct BootConfig {
	/// The total number of cores being booted.
	pub core_count: u64,
	/// The physical address of the top of the page frame allocator stack.
	pub pfa_stack_top: u64,
}
