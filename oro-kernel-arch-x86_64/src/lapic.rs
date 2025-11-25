//! Provides the Local APIC (Advanced Programmable Interrupt Controller)
//! implementation for the Oro kernel.
//!
//! Documentation found in Section 11 of the Intel SDM Volume 3A.

use oro_arch_x86_64::lapic::{ApicSvr, ApicTimerConfig, ApicTimerDivideBy, ApicTimerMode};

/// The vector number for the APIC spurious interrupt.
const APIC_SVR_VECTOR: u8 = 0xFF;
/// The vector number for the system timer interrupt.
const PIT_TIMER_VECTOR: u8 = 0x20;

/// Initializes the APIC (Advanced Programmable Interrupt Controller)
/// for interrupt handling.
///
/// # Safety
/// Modifies global state, and must be called only once per core.
///
/// The kernel MUST be fully initialized before calling this function.
pub unsafe fn initialize_lapic_irqs() {
	let lapic = &crate::Kernel::get().handle().lapic;

	lapic.set_spurious_vector(
		ApicSvr::new()
			.with_vector(APIC_SVR_VECTOR)
			.with_software_enable(),
	);

	lapic.set_timer_divider(ApicTimerDivideBy::Div128);

	lapic.configure_timer(
		ApicTimerConfig::new()
			.with_vector(PIT_TIMER_VECTOR)
			.with_mode(ApicTimerMode::OneShot),
	);
}
