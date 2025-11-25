//! The Interrupt Descriptor Table (IDT) and related structures.

use core::arch::asm;

/// A single IDT (Interrupt Descriptor Table) entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct IdtEntry {
	/// The lower 16 bits of the ISR (Interrupt Service Routine) address.
	isr_low:    u16,
	/// The code segment selector used when handling the interrupt.
	kernel_cs:  u16,
	/// The IST (Interrupt Stack Table) index.
	ist:        u8,
	/// Attributes and flags for the IDT entry, including type.
	attributes: u8,
	/// The middle 16 bits of the ISR address.
	isr_mid:    u16,
	/// The higher 32 bits of the ISR address.
	isr_high:   u32,
	/// Reserved.
	_reserved:  u32,
}

impl IdtEntry {
	/// Creates a new, empty IDT entry.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			isr_low:    0,
			kernel_cs:  0,
			ist:        0,
			attributes: 0,
			isr_mid:    0,
			isr_high:   0,
			_reserved:  0,
		}
	}

	/// Sets the ISR address for the IDT entry.
	///
	/// # Safety
	/// Caller must ensure that the given address is
	/// a real function that is suitable for handling
	/// the interrupt.
	#[must_use]
	pub const unsafe fn with_isr_raw(mut self, isr: u64) -> Self {
		self.isr_low = isr as u16;
		self.isr_mid = (isr >> 16) as u16;
		self.isr_high = (isr >> 32) as u32;

		self
	}

	/// Sets the ISR handler as a function pointer.
	#[must_use]
	pub fn with_isr(self, isr: unsafe extern "C" fn() -> !) -> Self {
		unsafe { self.with_isr_raw(isr as usize as u64) }
	}

	/// Sets the code segment selector for the IDT entry.
	///
	/// There is no configurable index for this; it's always
	/// set to 0x08 (the kernel code segment).
	#[must_use]
	pub const fn with_kernel_cs(mut self) -> Self {
		self.kernel_cs = crate::gdt::KERNEL_CS;
		self
	}

	/// Sets the attributes for the IDT entry.
	#[must_use]
	pub const fn with_attributes(mut self, attributes: u8) -> Self {
		self.attributes = attributes;
		self
	}
}

/// Installs the given IDT (Interrupt Descriptor Table).
///
/// # Safety
/// Modifies CPU mode and global state.
pub unsafe fn install_idt(idt: &'static [IdtEntry]) {
	/// The IDTR (Interrupt Descriptor Table Register) structure,
	/// read in by the `lidt` instruction.
	#[repr(C, packed)]
	struct Idtr {
		/// How long the IDT is in bytes, minus 1.
		limit: u16,
		/// The base address of the IDT.
		base:  *const IdtEntry,
	}

	debug_assert!(
		u16::try_from(size_of_val(idt)).is_ok(),
		"given IDT is too large"
	);

	#[allow(static_mut_refs)]
	let idtr = Idtr {
		limit: (size_of_val(idt) - 1) as u16,
		base:  &raw const idt[0],
	};

	// We load the IDT as early as possible, prior to telling the APIC
	// it can fire events at us.
	// SAFETY: We ensure that the IDT is valid and properly formed.
	unsafe {
		asm!(
			"lidt [{}]",
			in(reg) &idtr,
			options(nostack)
		);
	}
}
