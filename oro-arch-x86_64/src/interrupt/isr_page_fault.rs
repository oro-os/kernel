//! The page fault exception ISR (Interrupt Service Routine).

use core::arch::asm;
use oro_kernel::scheduler::PageFaultType;
use oro_sync::Lock;

crate::isr! {
	/// The ISR (Interrupt Service Routine) for page fault exceptions.
	unsafe fn isr_page_fault(kernel, user_task, err_code) -> Option<Switch> {
		let cr2: usize;
		// SAFETY: `cr2` is a register that holds the faulting address. It is safe to read.
		unsafe {
			asm!("mov {}, cr2", out(reg) cr2, options(nostack, preserves_flags));
		}

		// Decode the error code.
		// TODO(qix-): Use a register definition for this.
		let is_ifetch = err_code & (1 << 4) != 0;
		let is_user = err_code & (1 << 2) != 0;
		let is_write = err_code & (1 << 1) != 0;

		if !is_user {
			panic!("kernel page fault: err={err_code:#032b} cr2={cr2:#016X} core={}", kernel.id());
		}

		let err_type = if is_ifetch {
			PageFaultType::Execute
		} else if is_write {
			PageFaultType::Write
		} else {
			PageFaultType::Read
		};

		// SAFETY: `event_page_fault` specifies that, in the event we return back to the task,
		// SAFETY: that the task has been instructed to re-try the memory operation. x86_64
		// SAFETY: does this by design, so we must do no special handling here.
		Some(unsafe { kernel.scheduler().lock().event_page_fault(err_type, cr2) })
	}
}
