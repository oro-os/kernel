//! Implements the kernel's interrupt handling logic.
//!
//! This is the late stage interrupt system that interacts
//! with an established kernel instance, usable to shuttle
//! userspace events to the kernel's scheduler.

use crate::lapic::{ApicSvr, ApicTimerConfig, ApicTimerMode};

crate::isr_table! {
	/// The IDT (Interrupt Descriptor Table) for the kernel.
	static IDT = {
		{ super::default::get_default_isr_table() }

		// TODO(qix-): Exception ISRs (which cannot be processed with the default ISR).
		PAGE_FAULT[14] => isr_page_fault,
		TIMER_VECTOR[32] => isr_sys_timer,
		APIC_SVR_VECTOR[255] => isr_apic_svr,
		_ => default_isr,
	};
}

/// Initializes the APIC (Advanced Programmable Interrupt Controller)
/// for interrupt handling.
///
/// # Safety
/// Modifies global state, and must be called only once.
///
/// The kernel MUST be fully initialized before calling this function.
pub unsafe fn initialize_lapic_irqs() {
	let lapic = &crate::Kernel::get().handle().lapic;

	lapic.set_spurious_vector(
		ApicSvr::new()
			.with_vector(APIC_SVR_VECTOR)
			.with_software_enable(),
	);

	lapic.set_timer_divider(crate::lapic::ApicTimerDivideBy::Div128);

	lapic.configure_timer(
		ApicTimerConfig::new()
			.with_vector(TIMER_VECTOR)
			.with_mode(ApicTimerMode::OneShot),
	);
}

/// Installs the kernel IDT (Interrupt Descriptor Table) for the kernel
/// and enables interrupts.
///
/// # LAPIC / Spurious Interrupts
/// This function **does not** initialize the LAPIC (Local APIC) for
/// interrupt handling. This must be done separately with
/// [`initialize_lapic_irqs`].
///
/// # Safety
/// Modifies global state, and must be called only once - preferably
/// early, and after the GDT has been installed.
pub unsafe fn install_kernel_idt() {
	// SAFETY: Safety has been ensured by the caller.
	crate::interrupt::install_idt(IDT.get());
}
