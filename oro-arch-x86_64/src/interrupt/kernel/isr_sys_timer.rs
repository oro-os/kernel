//! System timer interrupt handler
use ::oro_sync::Lock;

crate::isr! {
	/// The ISR (Interrupt Service Routine) for the system timer.
	unsafe fn isr_sys_timer(kernel, _user_task) -> Option<Switch> {
		kernel.handle().lapic.eoi();
		Some(kernel.scheduler().lock().event_timer_expired())
	}
}
