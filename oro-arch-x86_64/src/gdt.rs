//! Structures and implementations for managing
//! descriptor tables and their entries.

use core::arch::asm;

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

/// The GDT.
static GDT: [GdtEntry; 5] = [
	GdtEntry::null_descriptor(),
	GdtEntry::kernel_code_segment(), // kernel code MUST be index 1
	GdtEntry::kernel_data_segment(),
	GdtEntry::user_code_segment(),
	GdtEntry::user_data_segment(),
];

/// Returns a byte slice of the GDT.
///
/// This is mostly used by the secondary core initialization
/// code to write the GDT to a 32-bit page, as is required
/// when running in a 16/32-bit mode.
#[must_use]
pub fn gdt_bytes() -> &'static [u8] {
	// SAFETY(qix-): The GDT is a static array, so it's always valid.
	unsafe { core::slice::from_raw_parts(GDT.as_ptr().cast::<u8>(), core::mem::size_of_val(&GDT)) }
}

/// Installs the GDT.
pub fn install_gdt() {
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

	let base = GDT.as_ptr() as u64;
	let gdt_size = core::mem::size_of_val(&GDT);
	#[allow(clippy::cast_possible_truncation)]
	let limit = (gdt_size - 1) as u16;

	let gdt_descriptor = GdtDescriptor { limit, base };

	// SAFETY(qix-): The GDT is a static array, so it's always valid.
	// SAFETY(qix-): There's also nothing about marking this function
	// SAFETY(qix-): as 'unsafe' that would prevent a crash on incorrect
	// SAFETY(qix-): GDT configuration.
	unsafe {
		asm! {
			// Load the GDT.
			"lgdt [{0}]",
			// Set up code segment.
			// CS is at offset 0x08, and we can't just move into CS,
			// so we must push the segment selector onto the stack and
			// then return to it.
			"sub rsp, 16",
			"mov qword ptr[rsp + 8], 0x08",
			"lea rax, [rip + 2f]",
			"mov qword ptr[rsp], rax",
			"retfq",
			// Using 2f instead of 0/1 due to LLVM bug
			// (https://bugs.llvm.org/show_bug.cgi?id=36144)
			// causing them to be parsed as binary literals
			// under intel syntax.
			"2:",
			// Set up non-code segments.
			"mov ax, 0x10",
			"mov ds, ax",
			"mov es, ax",
			"mov fs, ax",
			"mov gs, ax",
			"mov ss, ax",
			in(reg) &gdt_descriptor
		};
	}
}
