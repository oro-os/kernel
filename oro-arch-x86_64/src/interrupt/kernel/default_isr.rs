//! Implements a "default" ISR (Interrupt Service Routine) that will be called by the kernel
//! when the corresponding interrupt is triggered.

crate::isr! {
	/// The ISR (Interrupt Service Routine) for the APIC spurious interrupt.
	unsafe fn default_isr(kernel, _user_task) -> Option<Switch> {
		kernel.handle().lapic.eoi();
		None
	}
}
