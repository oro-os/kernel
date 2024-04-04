//! Defines the Oro Operating System address space layout for `x86_64` CPUs.
#![allow(clippy::inline_always)]

use crate::mem::paging::PageTableEntry;
use oro_common::mem::AddressSpaceLayout;

/// Holds initialization and range information for an address space segment
/// for the `x86_64` architecture address space.
pub struct Descriptor {
	/// The valid range of L4/L5 indices for this segment.
	pub valid_range:    (usize, usize),
	/// A template for the page table entry to use for this segment.
	pub entry_template: PageTableEntry,
}

/// Defines the layout of the address space for the `x86_64` architecture.
pub struct Layout;

impl Layout {
	/// The recursive index for the page table.
	pub const RECURSIVE_IDX: usize = 256;
}

/// The kernel executable range, shared by the RX, RO, and RW segments.
const KERNEL_EXE: (usize, usize) = (511, 511);

unsafe impl AddressSpaceLayout for Layout {
	#![allow(clippy::missing_docs_in_private_items)]

	type Descriptor = &'static Descriptor;

	#[inline(always)]
	fn kernel_code() -> Self::Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    KERNEL_EXE,
			entry_template: PageTableEntry::new()
				.with_user()
				.with_global()
				.with_present(),
		};

		&DESCRIPTOR
	}

	#[inline(always)]
	fn kernel_data() -> Self::Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    KERNEL_EXE,
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}

	#[inline(always)]
	fn kernel_rodata() -> Self::Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    KERNEL_EXE,
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec(),
		};

		&DESCRIPTOR
	}
}
