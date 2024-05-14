//! Defines the Oro Operating System address space layout for x86_64 CPUs.
#![allow(clippy::inline_always)]

use super::paging_level::PagingLevel;
use crate::mem::paging::PageTableEntry;
use oro_common::mem::{AddressRange, AddressSpaceLayout};

/// Holds initialization and range information for an address space segment
/// for the x86_64 architecture address space.
pub struct Descriptor {
	/// The valid range of L4/L5 indices for this segment.
	pub valid_range:    (usize, usize),
	/// A template for the page table entry to use for this segment.
	pub entry_template: PageTableEntry,
}

/// Sign-extends a value to the appropriate size for the current paging level.
macro_rules! sign_extend {
	(L4, $value:expr) => {
		((($value << 16) as isize) >> 16) as usize
	};
	(L5, $value:expr) => {
		((($value << 7) as isize) >> 7) as usize
	};
}

impl AddressRange for Descriptor {
	#[inline(always)]
	fn valid_range(&self) -> (usize, usize) {
		// Get the current paging level.
		match PagingLevel::current_from_cpu() {
			PagingLevel::Level4 => {
				(
					sign_extend!(L4, self.valid_range.0 << 39),
					sign_extend!(L4, (self.valid_range.1 << 39) | 0x0000_007F_FFFF_FFFF),
				)
			}
			PagingLevel::Level5 => {
				(
					sign_extend!(L5, self.valid_range.0 << 48),
					sign_extend!(L5, (self.valid_range.1 << 48) | 0x0000_FFFF_FFFF_FFFF),
				)
			}
		}
	}
}

/// Defines the layout of the address space for the x86_64 architecture.
pub struct Layout;

impl Layout {
	/// The direct map range
	pub const DIRECT_MAP_IDX: (usize, usize) = (258, 300);
	/// The kernel executable range, shared by the RX, RO, and RW segments.
	pub const KERNEL_EXE_IDX: usize = 511;
	/// The stack space range
	pub const KERNEL_STACK_IDX: usize = 257;
	/// The recursive index for the page table.
	pub const RECURSIVE_IDX: usize = 256;
	/// The index for kernel transfer stubs.
	pub const STUBS_IDX: usize = 255;

	#[inline(always)]
	pub fn stubs() -> &'static Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    (Layout::STUBS_IDX, Layout::STUBS_IDX),
			entry_template: PageTableEntry::new().with_present().with_writable(),
		};

		&DESCRIPTOR
	}

	#[inline(always)]
	pub fn kernel_stack() -> &'static Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    (Layout::KERNEL_STACK_IDX, Layout::KERNEL_STACK_IDX),
			entry_template: PageTableEntry::new()
				.with_present()
				.with_writable()
				.with_no_exec(),
		};

		&DESCRIPTOR
	}
}

unsafe impl AddressSpaceLayout for Layout {
	#![allow(clippy::missing_docs_in_private_items)]

	type Descriptor = &'static Descriptor;

	#[inline(always)]
	fn kernel_code() -> Self::Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    (Layout::KERNEL_EXE_IDX, Layout::KERNEL_EXE_IDX),
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
			valid_range:    (Layout::KERNEL_EXE_IDX, Layout::KERNEL_EXE_IDX),
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
			valid_range:    (Layout::KERNEL_EXE_IDX, Layout::KERNEL_EXE_IDX),
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec(),
		};

		&DESCRIPTOR
	}

	#[inline(always)]
	fn direct_map() -> Self::Descriptor {
		const DESCRIPTOR: Descriptor = Descriptor {
			valid_range:    Layout::DIRECT_MAP_IDX,
			entry_template: PageTableEntry::new()
				.with_global()
				.with_present()
				.with_no_exec()
				.with_writable(),
		};

		&DESCRIPTOR
	}
}
