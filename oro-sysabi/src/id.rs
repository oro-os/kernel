//! Oro module metadata and ABI constants.

/// ID masks for kernel interfaces.
pub mod mask {
	/// `(id & KERNEL_ID) == 0` indicates a kernel ID.
	///
	/// Any other ID is a non-standard, user-defined ID.
	pub const KERNEL_ID: u64 = 0xFFFF_FFFF_0000_0000;

	/// `(id & KERNEL_ID_TYPE)` extracts the kernel ID type.
	///
	/// Note that this _does_ include the high 32-bits
	/// so that any erroneously operated upon user-defined
	/// ID will not somehow pass the check.
	pub const KERNEL_ID_TYPE: u64 = 0xFFFF_FFFF_FF00_0000;

	/// `(iface & KERNEL_ID_TYPE) == KERNEL_ID_TYPE_PRIMITIVE` indicates a primitive type.
	pub const KERNEL_ID_TYPE_PRIMITIVE: u64 = 0x0100_0000;

	/// `(iface & KERNEL_ID_TYPE) == KERNEL_ID_TYPE_IFACE` indicates a kernel interface.
	pub const KERNEL_ID_TYPE_IFACE: u64 = 0x0200_0000;

	/// `(iface & KERNEL_ID_TYPE) == KERNEL_ID_TYPE_META` indicates a module metadata structure.
	pub const KERNEL_ID_TYPE_META: u64 = 0x0300_0000;
}

/// Kernel interface IDs.
#[expect(clippy::unreadable_literal)]
pub mod iface {
	use crate::id::mask::KERNEL_ID_TYPE_IFACE;

	/// The ID of the kernel threading interface (version 0).
	pub const THREAD_V0: u64 = KERNEL_ID_TYPE_IFACE | 0x00_001;

	/// The ID of the root ring debug output interface (version 0).
	pub const ROOT_DEBUG_OUT_V0: u64 = 1736981805247;
}

/// Kernel primitive type IDs.
pub mod primitive {
	use crate::id::mask::KERNEL_ID_TYPE_PRIMITIVE;

	/// The ID of the kernel `usize` primitive type.
	pub const U64: u64 = KERNEL_ID_TYPE_PRIMITIVE | 0x00_001;
}

/// Kernel metadata IDs.
pub mod meta {
	use crate::id::mask::KERNEL_ID_TYPE_META;

	/// ID indicating that the following metadata indicates an interface/key usage.
	pub const USES: u64 = KERNEL_ID_TYPE_META | 0x00_001;

	/// ID indicating that the following metadata indicates an interface slot.
	pub const IFACE_SLOT: u64 = KERNEL_ID_TYPE_META | 0x00_002;
}
