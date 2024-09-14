//! Houses types, traits and functionality for the Oro kernel scheduler.

use crate::{Arch, UserHandle};
use core::marker::PhantomData;

/// Architecture-specific handler for scheduler related
/// commands.
///
/// Upon events coming into the CPU, the architecture will
/// consult the [`crate::Kernel`] about what to do next.
///
/// The kernel will accept an object bounded to this trait,
/// through which it may issue commands to the architecture
/// to perform certain low-level actions, before finally
/// returning a user context to which the architecture will
/// switch to.
///
/// During the time the kernel handler is processing, the
/// kernel thread may execute tasks related to system management,
/// kernel modules, etc. before handing control back to
/// a userspace thread via the architecture.
pub trait Handler {
	/// Tells a one-off timer to expire after `ticks`.
	/// The architecture should not transform the number
	/// of ticks unless it has good reason to.
	///
	/// The architecture should call [`Scheduler::event_timer_expired()`]
	/// if the timer expires.
	fn schedule_timer(&self, ticks: u32);

	/// Tells the architecture to cancel any pending timer.
	///
	/// Between this point and a subsequent call to
	/// [`Self::schedule_timer()`], the architecture should
	/// not call [`Scheduler::event_timer_expired()`].
	fn cancel_timer(&self);
}

/// Main scheduler state machine.
///
/// This type is separated out from the [`crate::Kernel`]
/// for the sake of modularity and separation of concerns.
///
/// This structure houses all of the necessary state and
/// functionality to manage the scheduling of tasks within
/// the Oro kernel, including that of the kernel thread
/// itself.
pub struct Scheduler<A: Arch> {
	/// The architecture
	_arch: PhantomData<A>,
}

impl<A: Arch> Scheduler<A> {
	/// Creates a new scheduler instance.
	pub(crate) fn new() -> Self {
		Self { _arch: PhantomData }
	}

	/// Called whenever the architecture has reached a codepath
	/// where it's not sure what to do next (e.g. the first thing
	/// at boot).
	///
	/// # Interrupt Safety
	/// This function is safe to call from an interrupt context,
	/// though it is _not_ explicitly required to be called from
	/// such a context.
	///
	/// It must be called with the original kernel stack in place,
	/// and must run in the supervisor's context (including any
	/// permissions levels relevant for supervisory instructions
	/// to execute without access faults, as well as the kernel
	/// memory map being intact).
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.** At no point
	/// can other scheduler methods be invoked while this function
	/// is running.
	pub unsafe fn event_idle<H: Handler>(&self, handler: &H) -> Option<UserHandle<A>> {
		// XXX TODO(qix-): debug placeholder.
		handler.schedule_timer(10000);
		None
	}

	/// Indicates to the kernel that the system timer has fired,
	/// indicating the end of a time slice.
	///
	/// Returns either a userspace handle to switch to and continue
	/// executing, or `None` if the architecture should enter
	/// a low-power / wait state until an interrupt or event occurs.
	///
	/// # Interrupt Safety
	/// This function is safe to call from an interrupt context,
	/// though it is _not_ explicitly required to be called from
	/// such a context.
	///
	/// It must be called with the original kernel stack in place,
	/// and must run in the supervisor's context (including any
	/// permissions levels relevant for supervisory instructions
	/// to execute without access faults, as well as the kernel
	/// memory map being intact).
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.** At no point
	/// can other scheduler methods be invoked while this function
	/// is running.
	pub unsafe fn event_timer_expired<H: Handler>(&self, handler: &H) -> Option<UserHandle<A>> {
		// XXX TODO(qix-): debug placeholder.
		handler.schedule_timer(10000);
		None
	}
}
