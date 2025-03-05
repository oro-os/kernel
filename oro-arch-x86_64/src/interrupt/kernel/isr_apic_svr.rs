//! Spurious interrupt vector register for the APIC.

crate::isr! {
	/// The ISR (Interrupt Service Routine) for the APIC spurious interrupt.
	unsafe fn isr_apic_svr(kernel, _user_task) -> Option<Switch> {
		kernel.handle().lapic.eoi();
		None
	}
}
