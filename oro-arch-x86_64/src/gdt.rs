//! Structures and implementations for managing
//! descriptor tables and their entries.

use core::arch::asm;

use crate::tss::Tss;

/// A global descriptor table (GDT) entry.
///
/// Note that task state segment (TSS) entries are
/// ultimately stored in the GDT as two GDT entries,
/// so the introspection of GDT entries is not recommended
/// unless you know the GDT layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
#[repr(transparent)]
pub struct GdtEntry(u64);

// NOTE(qix-): Most fields are ignored in 64-bit mode, so
// NOTE(qix-): mutators aren't added here.
// TODO(qix-): Make these mutators mask off the bits before applying them.
impl GdtEntry {
	/// Returns a null descriptor, used as the first
	/// entry in the GDT.
	pub const fn null_descriptor() -> Self {
		Self(0)
	}

	/// Returns a new GDT entry with the 'descriptor type'
	/// bit set.
	pub const fn new() -> Self {
		Self(1 << 44)
	}

	/// Returns the kernel code segment descriptor
	/// for the x86_64 architecture.
	pub const fn kernel_code_segment() -> Self {
		Self::new()
			.with_present()
			.with_accessed()
			.with_long_mode()
			.with_ring(Dpl::Ring0)
			.with_rw()
			.with_executable()
	}

	/// Returns the kernel data segment descriptor
	/// for the x86_64 architecture.
	pub const fn kernel_data_segment() -> Self {
		Self::new()
			.with_present()
			.with_accessed()
			.with_rw()
			.with_long_mode()
			.with_ring(Dpl::Ring0)
	}

	/// Returns the user code segment descriptor
	/// for the x86_64 architecture.
	pub const fn user_code_segment() -> Self {
		Self::new()
			.with_present()
			.with_accessed()
			.with_long_mode()
			.with_ring(Dpl::Ring3)
			.with_rw()
			.with_executable()
			.with_conforming()
	}

	/// Returns the user data segment descriptor
	/// for the x86_64 architecture.
	pub const fn user_data_segment() -> Self {
		Self::new()
			.with_present()
			.with_accessed()
			.with_rw()
			.with_long_mode()
			.with_ring(Dpl::Ring3)
			.with_conforming()
	}

	/// Setting this flag will prevents the GDT from
	/// writing to the segment on first use.
	pub const fn with_accessed(self) -> Self {
		Self(self.0 | (1 << 40))
	}

	/// Setting this flag allows the segment to be
	/// written to (if it's a data segment) or read
	/// from (if it's a code segment).
	pub const fn with_rw(self) -> Self {
		Self(self.0 | (1 << 41))
	}

	/// Setting this flag allows the segment to be
	/// executed. Must be set for CS and unset for DS.
	pub const fn with_executable(self) -> Self {
		Self(self.0 | (1 << 43))
	}

	/// Sets the DPL (Data Privilege Level) for the
	/// descriptor. This corresponds to the ring level
	/// of the descriptor.
	pub const fn with_ring(self, ring: Dpl) -> Self {
		Self(self.0 | ((ring as u64) << 45))
	}

	/// Sets the present bit for the descriptor.
	/// Must be set for all valid descriptors.
	pub const fn with_present(self) -> Self {
		Self(self.0 | (1 << 47))
	}

	/// Sets the long mode bit for the descriptor.
	/// Must be set for all valid descriptors.
	pub const fn with_long_mode(self) -> Self {
		Self(self.0 | (1 << 53))
	}

	/// Allows rings lower than or equal to the CPL
	/// to interact with this segment.
	pub const fn with_conforming(self) -> Self {
		Self(self.0 | (1 << 42))
	}
}

/// A Data Privilege Level (DPL) for a descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Dpl {
	/// Ring 0.
	Ring0 = 0,
	/// Ring 1.
	Ring1 = 1,
	/// Ring 2.
	Ring2 = 2,
	/// Ring 3.
	Ring3 = 3,
}

/// A system segment (task state segment (TSS) or LDT) entry for x86_64.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(clippy::missing_docs_in_private_items)]
#[must_use]
#[repr(C, align(8))]
pub struct SysEntry {
	low:  u64,
	high: u64,
}

impl SysEntry {
	/// Creates a new TSS entry.
	pub const fn new() -> Self {
		Self { low: 0, high: 0 }
	}

	/// Creates a basic TSS entry for DPL 0 with the given pointer to
	/// a [`Tss`] structure.
	pub fn for_tss(tss: *const Tss) -> Self {
		Self::new()
			.with_granularity(true)
			.with_long_mode()
			.with_limit(size_of::<Tss>() as u32 - 1)
			.with_present()
			.with_type(SysType::TssAvail)
			.with_ring(Dpl::Ring0)
			.with_base(tss as u64)
	}

	/// Sets the base address of the system segment.
	pub const fn with_base(self, base: u64) -> Self {
		Self {
			low:  (self.low & 0x00FF_FF00_0000_FFFF)
				| ((base & 0x00FF_FFFF) << 16)
				| ((base & 0xFF00_0000) << 56),
			high: base >> 32,
		}
	}

	/// Sets the limit of the system segment.
	///
	/// Bits higher than 19 are ignored.
	pub const fn with_limit(self, limit: u32) -> Self {
		Self {
			low:  (self.low & 0xFFF0_FFFF_FFFF_0000)
				| (limit as u64 & 0xFFFF)
				| ((limit as u64 & 0xF0000) << 32),
			high: self.high,
		}
	}

	/// Sets the type of the system segment.
	pub const fn with_type(self, ty: SysType) -> Self {
		Self {
			low:  (self.low & 0xFFFF_F0FF_FFFF_FFFF) | ((ty as u64) << 40),
			high: self.high,
		}
	}

	/// Sets the DPL (Data Privilege Level) for the
	/// descriptor. This corresponds to the ring level
	/// of the descriptor.
	pub const fn with_ring(self, ring: Dpl) -> Self {
		Self {
			low:  (self.low & 0xFFFF_9FFF_FFFF_FFFF) | ((ring as u64) << 45),
			high: self.high,
		}
	}

	/// Sets the present bit for the descriptor.
	/// Must be set for all valid descriptors.
	pub const fn with_present(self) -> Self {
		Self {
			low:  self.low | (1 << 47),
			high: self.high,
		}
	}

	/// Sets the granularity of the descriptor.
	///
	/// If `true`, the limit is in 4KiB blocks.
	/// If `false`, the limit is in bytes.
	pub const fn with_granularity(self, gran: bool) -> Self {
		Self {
			low:  (self.low & 0xFF7F_FFFF_FFFF_FFFF) | ((gran as u64) << 55),
			high: self.high,
		}
	}

	/// Sets the long mode bit for the descriptor.
	/// Must be set for all valid descriptors.
	pub const fn with_long_mode(self) -> Self {
		Self {
			low:  self.low | (1 << 53),
			high: self.high,
		}
	}
}

/// A system segment type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum SysType {
	/// A local descriptor table (LDT) entry.
	Ldt      = 0x2,
	/// 64-bit TSS (task state segment), available.
	TssAvail = 0x9,
	/// 64-bit TSS (task state segment), busy.
	TssBusy  = 0xB,
}

/// A basic GDT that can be used for early-stage booting.
pub static GDT: Gdt<6> = Gdt::<6>::new();

/// A global descriptor table (GDT).
#[must_use]
#[repr(C, align(16))]
pub struct Gdt<const COUNT: usize> {
	/// The entries in the GDT.
	entries: [GdtEntry; COUNT],
}

/// The offset into the standard GDT of the kernel code segment.
pub const KERNEL_CS: u16 = 0x08;
/// The offset into the standard GDT of the kernel data segment.
pub const KERNEL_DS: u16 = 0x10;
/// The offset into the standard GDT of the user code segment.
pub const USER_CS: u16 = 0x18;
/// The offset into the standard GDT of the user data segment.
pub const USER_DS: u16 = 0x20;
/// The offset of the sysem call STAR GDT entry for kernel mode.
pub const STAR_KERNEL: u16 = KERNEL_CS;
/// The offset of the system call STAR GDT entry for user mode.
///
/// The offset here must be formatted such that
/// - `STAR+0` is the user CS
/// - `STAR+8` is the user SS (DS)
/// - `STAR+16` is the user CS (again)
pub const STAR_USER: u16 = USER_CS;

/// Where the Task State Segment (TSS) should be placed in the GDT.
///
/// Verified at boot time, such that this index can be used without having
/// to perform a lookup.
pub const TSS_GDT_OFFSET: u16 = 0x30;

impl<const COUNT: usize> Gdt<COUNT> {
	/// Creates a new GDT with the standard entries (see the `*_CS` and `*_DS` constants).
	pub const fn new() -> Gdt<6> {
		Gdt {
			// MUST match the `*_CS` and `*_DS` constants above.
			entries: [
				GdtEntry::null_descriptor(),
				// Must be in the order CS, DS for compatibility with STAR[47:32].
				GdtEntry::kernel_code_segment(),
				GdtEntry::kernel_data_segment(),
				// Must be in the order CS, DS, CS for compatibility with STAR[63:48].
				GdtEntry::user_code_segment(),
				GdtEntry::user_data_segment(),
				// Repeated here (must be directly after user data segment)
				// since SYSRET loads STAR[63:48]+16. See `STAR` constant.
				GdtEntry::user_code_segment(),
			],
		}
	}

	/// Returns a new GDT with the given entry added.
	///
	/// Returns the offset (in bytes) of the entry and the new GDT as a tuple.
	pub const fn with_entry(self, entry: GdtEntry) -> (u16, Gdt<{ COUNT + 1 }>) {
		#[repr(C)]
		#[derive(Copy, Clone)]
		struct Concat<A, B>(A, B);
		let concat = Concat(self.entries, entry);
		(
			// TODO(qix-): When const traits are eventually stabilized
			// TODO(qix-): (a ways off), change the `as u16` line to this:
			// TODO(qix-): u16::try_from(COUNT * 0x08).expect("GDT too big"),
			(COUNT * 0x08) as u16,
			Gdt {
				entries: unsafe { core::mem::transmute_copy(&concat) },
			},
		)
	}

	/// Returns a new GDT with the given system entry added.
	///
	/// Returns the offset (in bytes) of the entry and the new GDT as a tuple.
	pub const fn with_sys_entry(self, entry: SysEntry) -> (u16, Gdt<{ COUNT + 2 }>) {
		#[repr(C)]
		#[derive(Copy, Clone)]
		struct Concat<A, B>(A, B);

		let low = GdtEntry(entry.low);
		let high = GdtEntry(entry.high);
		let concat = Concat(self.entries, [low, high]);

		(
			// TODO(qix-): When const traits are eventually stabilized
			// TODO(qix-): (a ways off), change the `as u16` line to this:
			// TODO(qix-): u16::try_from(COUNT * 0x08).expect("GDT too big"),
			(COUNT * 0x08) as u16,
			Gdt {
				entries: unsafe { core::mem::transmute_copy(&concat) },
			},
		)
	}

	/// Returns a byte slice of the GDT.
	///
	/// This is mostly used by the secondary core initialization
	/// code to write the GDT to a 32-bit page, as is required
	/// when running in a 16/32-bit mode.
	#[must_use]
	pub fn as_bytes(&self) -> &'static [u8] {
		// SAFETY(qix-): The GDT is a static array, so it's always valid.
		unsafe {
			core::slice::from_raw_parts(self.entries.as_ptr().cast::<u8>(), size_of_val(&GDT))
		}
	}

	/// Installs the GDT.
	///
	/// # Safety
	/// The GDT must not be moved and remain valid until a new GDT is installed.
	///
	/// If a new GDT is never installed, `self` must exist at the same address
	/// for the entire duration of the system's operation.
	pub unsafe fn install(&self) {
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

		let base = self.entries.as_ptr() as u64;
		let gdt_size = size_of_val(self);
		#[expect(clippy::cast_possible_truncation)]
		let limit = (gdt_size - 1) as u16;

		let gdt_descriptor = GdtDescriptor { limit, base };

		// SAFETY(qix-): The offsets in this function must only refer
		// SAFETY(qix-): to the forced offsets defined in `Gdt`.
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
			"mov ss, ax",
			// Make sure that fs/gs segment registers are NULL descriptors.
			"mov ax, 0",
			"mov fs, ax",
			"mov gs, ax",
			in(reg) &gdt_descriptor,
			out("rax") _,
		};
	}
}
