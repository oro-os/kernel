//! Installer function for the IDT.

use core::arch::asm;

use crate::interrupt::idt::IdtEntry;

/// Installs the given IDT (Interrupt Descriptor Table).
///
/// # Safety
/// Modifies CPU mode and global state.
pub(crate) unsafe fn install_idt(idt: &'static [IdtEntry]) {
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
		u16::try_from(core::mem::size_of_val(idt)).is_ok(),
		"given IDT is too large"
	);

	#[allow(static_mut_refs)]
	let idtr = Idtr {
		limit: (core::mem::size_of_val(idt) - 1) as u16,
		base:  &raw const idt[0],
	};

	// We load the IDT as early as possible, prior to telling the APIC
	// it can fire events at us.
	asm!(
		"lidt [{}]",
		in(reg) &idtr,
		options(nostack)
	);
}
