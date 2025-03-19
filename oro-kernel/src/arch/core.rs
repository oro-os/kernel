//! Types and traits for implementing architecture-specific (CPU) core handles.

use core::{cell::UnsafeCell, time::Duration};

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
	/// A single 'instant' in time; used only for comparison of
	/// timestamps, etc.
	type Instant: Instant;

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

	/// Returns the current timestamp of the system.
	///
	/// This timestamp **must** be up to date at least at kernel event handler
	/// entry, and must always increase, never decrease.
	///
	/// This timestamp **does not** need to be the same clock source
	/// across all cores; each core may use its own clock source,
	/// as long as all other requirements are upheld.
	///
	/// This function may return the same timestamp across multiple calls
	/// within a single kernel context switch; it must be updated whenever
	/// a kernel event occurs (interrupt, system call, etc.), at the latest
	/// upon first call to this function within the kernel timeslice.
	///
	/// Further, this may return [`InstantResult::Overflow`] multiple times
	/// for the same kernel slice.
	fn now(&self) -> InstantResult<Self::Instant>;
}

/// A single 'instant' in time.
pub trait Instant: Sized + Clone + Copy + PartialEq + Eq + Ord {
	/// Adds the given [`Duration`] to this timestamp. If the timestamp overflows,
	fn checked_add(&self, duration: &Duration) -> InstantResult<Self>;

	/// Gets the time since the given instant that this instant occurred.
	///
	/// Returns `None` if the given instant is newer than this instant,
	/// or if the duration overflows a 64-bit nanosecond value.
	fn checked_duration_since(&self, other: &Self) -> Option<Duration>;
}

/// A result type for [`Instant`] queries and calculations.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InstantResult<I: Instant> {
	/// No wrapping occurred.
	Ok(I),
	/// The timestamp overflowed when calculating
	/// querying for it.
	///
	/// The given timestamp contains the remainder
	/// after the overflow, and thus will likely be
	/// **less than** a previous timestamp (but that
	/// is **not guaranteed**).
	Overflow(I),
}
