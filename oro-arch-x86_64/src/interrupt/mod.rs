//! Interrupt handling for x86_64 architecture.

use core::arch::asm;

use crate::lapic::{ApicSvr, ApicTimerConfig, ApicTimerMode};

mod macros;

crate::isr_table! {
	/// The IDT (Interrupt Descriptor Table) for the kernel.
	static IDT = {
		// TODO(qix-): Exception ISRs (which cannot be processed with the default ISR).
		PAGE_FAULT[14] => isr_page_fault,
		TIMER_VECTOR[32] => isr_sys_timer,
		APIC_SVR_VECTOR[255] => isr_apic_svr,
		_ => default_isr,
	};
}

/// Installs the IDT (Interrupt Descriptor Table) for the kernel
/// and enables interrupts.
///
/// # Safety
/// Modifies global state, and must be called only once.
///
/// The kernel MUST be fully initialized before calling this function.
pub unsafe fn install_idt() {
	// Get the LAPIC.
	let lapic = &crate::Kernel::get().handle().lapic;

	/// The IDTR (Interrupt Descriptor Table Register) structure,
	/// read in by the `lidt` instruction.
	#[repr(C, packed)]
	struct Idtr {
		/// How long the IDT is in bytes, minus 1.
		limit: u16,
		/// The base address of the IDT.
		base:  *const IdtEntry,
	}

	#[allow(static_mut_refs)]
	let idtr = Idtr {
		limit: (core::mem::size_of_val(&IDT.get().0) - 1) as u16,
		base:  &raw const IDT.get().0[0],
	};

	// We load the IDT as early as possible, prior to telling the APIC
	// it can fire events at us.
	asm!(
		"lidt [{}]",
		in(reg) &idtr,
		options(nostack, preserves_flags)
	);

	lapic.set_timer_divider(crate::lapic::ApicTimerDivideBy::Div128);

	lapic.configure_timer(
		ApicTimerConfig::new()
			.with_vector(TIMER_VECTOR)
			.with_mode(ApicTimerMode::OneShot),
	);

	lapic.set_spurious_vector(
		ApicSvr::new()
			.with_vector(APIC_SVR_VECTOR)
			.with_software_enable(),
	);
}

/// A single IDT (Interrupt Descriptor Table) entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct IdtEntry {
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
	pub const unsafe fn with_isr_raw(mut self, isr: u64) -> Self {
		self.isr_low = isr as u16;
		self.isr_mid = (isr >> 16) as u16;
		self.isr_high = (isr >> 32) as u32;

		self
	}

	/// Sets the ISR handler as a function pointer.
	pub fn with_isr(self, isr: unsafe extern "C" fn() -> !) -> Self {
		unsafe { self.with_isr_raw(isr as usize as u64) }
	}

	/// Sets the code segment selector for the IDT entry.
	///
	/// There is no configurable index for this; it's always
	/// set to 0x08 (the kernel code segment).
	pub const fn with_kernel_cs(mut self) -> Self {
		self.kernel_cs = 0x08;
		self
	}

	/// Sets the attributes for the IDT entry.
	pub const fn with_attributes(mut self, attributes: u8) -> Self {
		self.attributes = attributes;
		self
	}
}
