//! Types and traits for implementing architecture-specific (CPU) core handles.

use core::cell::UnsafeCell;

use crate::{arch::Arch, event::Resumption};

/// A handle to a local core.
///
/// Used primarily to issue timer and other core-wide operations.
///
/// # Safety
/// This trait is inherently unsafe. Implementors must take
/// great care that **all** invariants for **each individual method**
/// are upheld.
pub unsafe trait CoreHandle<A: Arch> {
	/// Tells a one-off timer to expire after `ticks`.
	/// The architecture should not transform the number
	/// of ticks unless it has good reason to.
	///
	/// The architecture should call [`crate::scheduler::Scheduler::event_timer_expired()`]
	/// if the timer expires.
	fn schedule_timer(&self, ticks: u32);

	/// Tells the core to cancel any pending timer.
	///
	/// Between this point and a subsequent call to
	/// [`Self::schedule_timer()`], the architecture should
	/// **not** call [`crate::scheduler::Scheduler::event_timer_expired()`].
	fn cancel_timer(&self);

	/// Runs the given context handle on the current core for the given number of ticks.
	///
	/// If the context is `None`, the core should halt for the given number of ticks.
	/// The implementation must not attempt to interpret the ticks in any way, and should
	/// pass the number directly to the underlying timing device (as the kernel has already
	/// calibrated itself to the timing of the device).
	///
	/// If the ticks is `None`, the context should run indefinitely, until it either yields
	/// or is preempted by a non-timer event.
	///
	/// If the resumption type is not `None`, the given data should be used to influence how
	/// and with which parameterization the context should be resumed. Implementers should
	/// always honor the resumption type, but may debug-assert if the resumption type doesn't
	/// meet stateful preconditions (e.g. the architecture may track, in debug mode, whether
	/// or not the thread was previously timer-preempted, but the resumption type is a system
	/// call response, etc.) - such cases are bugs in the kernel, as the kernel is specified
	/// as to properly track the state of the context.
	///
	/// This function should never return. Instead, a returning context switch should call
	/// [`crate::Kernel::handle_event()`] with the appropriate [`crate::event::PreemptionEvent`].
	///
	///
	/// # Safety
	/// Caller _must_ ensure that the context is not being run on any other core.
	///
	/// It is the responsibility of the implementation to properly update the context's state,
	/// if provided. The context is guaranteed only to be `None` if the kernel is idle and
	/// halting the core.
	unsafe fn run_context(
		&self,
		context: Option<&UnsafeCell<A::ThreadHandle>>,
		ticks: Option<u32>,
		resumption: Option<Resumption>,
	) -> !;
}
