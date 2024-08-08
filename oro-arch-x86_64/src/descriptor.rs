//! Structures and implementations for managing
//! descriptor tables and their entries.

use crate::mem::address_space::AddressSpaceLayout;
use oro_common::mem::mapper::AddressSegment;

/// A global descriptor table (GDT) entry.
#[repr(transparent)]
struct GdtEntry(u64);

// NOTE(qix-): Most fields are ignored in 64-bit mode, so
// NOTE(qix-): mutators aren't added here.
impl GdtEntry {
	/// Returns a null descriptor, used as the first
	/// entry in the GDT.
	const fn null_descriptor() -> Self {
		Self(0)
	}

	/// Returns the kernel code segment descriptor
	/// for the x86_64 architecture.
	const fn kernel_code_segment() -> Self {
		Self(0)
			.with_present()
			.with_accessed()
			.with_user()
			.with_writable()
			.with_long_mode()
			.with_ring(Dpl::Ring0)
			.with_executable()
	}

	/// Returns the kernel data segment descriptor
	/// for the x86_64 architecture.
	const fn kernel_data_segment() -> Self {
		Self(0)
			.with_present()
			.with_accessed()
			.with_user()
			.with_writable()
			.with_long_mode()
			.with_ring(Dpl::Ring0)
	}

	/// Returns the user code segment descriptor
	/// for the x86_64 architecture.
	const fn user_code_segment() -> Self {
		Self(0)
			.with_present()
			.with_accessed()
			.with_user()
			.with_writable()
			.with_long_mode()
			.with_ring(Dpl::Ring3)
			.with_executable()
	}

	/// Returns the user data segment descriptor
	/// for the x86_64 architecture.
	const fn user_data_segment() -> Self {
		Self(0)
			.with_present()
			.with_accessed()
			.with_user()
			.with_writable()
			.with_long_mode()
			.with_ring(Dpl::Ring3)
	}

	/// Setting this flag will prevents the GDT from
	/// writing to the segment on first use.
	const fn with_accessed(self) -> Self {
		Self(self.0 | 1 << 40)
	}

	/// Setting this flag allows the segment to be
	/// written to.
	const fn with_writable(self) -> Self {
		Self(self.0 | 1 << 41)
	}

	/// Setting this flag allows the segment to be
	/// executed. Must be set for CS and unset for DS.
	const fn with_executable(self) -> Self {
		Self(self.0 | 1 << 43)
	}

	/// Must be set for user segments.
	const fn with_user(self) -> Self {
		Self(self.0 | 1 << 44)
	}

	/// Sets the DPL (Data Privilege Level) for the
	/// descriptor. This corresponds to the ring level
	/// of the descriptor.
	const fn with_ring(self, ring: Dpl) -> Self {
		Self(self.0 | (ring as u64) << 45)
	}

	/// Sets the present bit for the descriptor.
	/// Must be set for all valid descriptors.
	const fn with_present(self) -> Self {
		Self(self.0 | 1 << 47)
	}

	/// Sets the long mode bit for the descriptor.
	/// Must be set for all valid descriptors.
	const fn with_long_mode(self) -> Self {
		Self(self.0 | 1 << 53)
	}
}

/// A Data Privilege Level (DPL) for a descriptor.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Dpl {
	/// Ring 0.
	Ring0 = 0,
	/// Ring 1.
	Ring1 = 1,
	/// Ring 2.
	Ring2 = 2,
	/// Ring 3.
	Ring3 = 3,
}

/// Writes the GDT to the given slice.
///
/// Writes the GDT descriptor first, followed by the GDT entries.
/// Thus, `lgdt` can be called with the kernel's eventual base
/// address of the slice.
///
/// # Safety
/// The slice must be 16-byte aligned.
///
/// It must also be at least a page long.
///
/// The slice must live exactly at the base range
/// of the [`AddressSpaceLayout::GDT_IDX`]
/// range when the kernel boots.
pub unsafe fn write_gdt(dest: &mut [u8]) {
	/// A GDT descriptor. Used exclusively by the `lgdt` instruction.
	///
	/// Must be packed, order matters.
	#[repr(C, packed(2))]
	struct GdtDescriptor {
		/// The limit. First, due to little-endian architecture.
		limit: u16,
		/// The base address of the GDT. Virtual, not physical.
		base:  u64,
	}

	let gdt = [
		GdtEntry::null_descriptor(),
		GdtEntry::kernel_code_segment(),
		GdtEntry::kernel_data_segment(),
		GdtEntry::user_code_segment(),
		GdtEntry::user_data_segment(),
	];

	// Calculate the GDT's base address.
	// It's the descriptor + whatever bytes are required
	// to align to 16 bytes.
	let gdt_base_offset = core::mem::size_of::<GdtDescriptor>();
	let gdt_base_offset = (gdt_base_offset + 15) & !15;

	let base = (AddressSpaceLayout::gdt().range().0 + gdt_base_offset) as u64;
	let gdt_size = gdt.len() * core::mem::size_of::<GdtEntry>();
	#[allow(clippy::cast_possible_truncation)]
	let limit = (gdt_size - 1) as u16;

	let gdt_descriptor = GdtDescriptor { limit, base };

	// Write the GDT descriptor first
	core::ptr::write_volatile(
		dest[..core::mem::size_of::<GdtDescriptor>()]
			.as_mut_ptr()
			.cast(),
		gdt_descriptor,
	);

	// Write the GDT entries
	core::ptr::write_volatile(
		dest[gdt_base_offset..(gdt_base_offset + gdt_size)]
			.as_mut_ptr()
			.cast(),
		gdt,
	);
}
