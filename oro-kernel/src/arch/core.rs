//! Types and traits for implementing architecture-specific (CPU) core handles.

use core::cell::UnsafeCell;

use crate::arch::Arch;

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
	/// The returned value is the event that occurred causing the context switch back to the
	/// kernel.
	///
	/// # Implementation Safety
	/// Implementors must treat this method as a singular call. This will typically be
	/// a complex entry point into userland, being returned to via an interrupt - an entirely
	/// separate path of execution.
	///
	/// Implementors must take extra care to fully restore the system state to what the Rust
	/// VM would expect upon return of this function, and this function must always return to
	/// the original caller as though it were a normal function call.
	///
	/// Implementors should not make any assumptions about the Rust ABI, and should try to return
	/// from this method using real Rust code rather than assembly stubs (i.e. returning to a
	/// controlled jump point, that ultimately resumes a function in the Rust portion of the implementation,
	/// followed by a Rust return).
	///
	/// Assertions are encouraged, to any extent possible, but it is urged that such assertions
	/// be debug-only. Unrecoverable errors should be handled in the form of a panic. **No userland
	/// code - error, exception, or otherwise - may result in an assertion, panic or UB.**
	///
	/// It is the responsibility of the implementation to properly update the context's state,
	/// if provided. The context is guaranteed only to be `None` if the kernel is idle and
	/// halting the core.
	///
	/// The response must always be returned by the originally executing core. Not doing so
	/// is undefined behavior.
	///
	/// # Safety
	/// Caller _must_ ensure that the context is not being run on any other core.
	unsafe fn run_context(
		&self,
		context: Option<&UnsafeCell<A::ThreadHandle>>,
		ticks: Option<u32>,
		resumption: Option<Resumption>,
	) -> PreemptionEvent;
}

/// A preemption event.
///
/// Returned by [`CoreHandle::run_context()`] to indicate the reason for the context switch
/// back to the kernel.
pub enum PreemptionEvent {
	/// The context was preempted by a timer event.
	Timer,
	/// The context invoked a system call.
	SystemCall(SystemCallRequest),
	/// The context page faulted.
	PageFault(PageFault),
	/// The context yielded.
	Yield,
	/// The context executed an invalid instruction.
	InvalidInstruction(InvalidInstruction),
}

/// A page fault preemption event.
///
/// Returned by [`PreemptionEvent::PageFault`] to indicate the reason for the page fault.
#[derive(Debug, Clone)]
pub struct PageFault {
	/// The address in memory that was accessed.
	pub address: usize,
	/// The faulting instruction address. If this information
	/// is not provided by the architecture, can be `None`.
	pub ip:      Option<usize>,
	/// The type of memory access.
	///
	/// See [`PageFaultAccess`] for information on how to
	/// choose a proper value for this field.
	pub access:  PageFaultAccess,
}

/// The type of memory access that caused a page fault.
///
/// # Choosing a Value
/// It's not assumed that a page fault is an exclusive operation;
/// however, the kernel only concerns itself with one operation at one time.
///
/// If multiple access types are involved in the fault, the following
/// rules should be followed:
///
/// - If the fault involves an execution attempt, regardless of any other
///   access types, the access type should be `Execute`.
/// - If the fault involves a write attempt, regardless if a read is also
///   involved, the access type should be `Write`.
/// - If the fault involves a read attempt, and no other access types are
///   involved, the access type should be `Read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultAccess {
	/// The fault was caused by an attempt to read from memory.
	Read,
	/// The fault was caused by an attempt to write to memory.
	Write,
	/// The fault was caused by an attempt to execute memory.
	Execute,
}

/// An invalid instruction preemption event.
pub struct InvalidInstruction {
	/// The faulting instruction address.
	pub ip: usize,
}

/// System call request data.
#[derive(Debug, Clone)]
pub struct SystemCallRequest {
	/// The opcode.
	pub opcode: u64,
	/// The first argument.
	pub arg1:   u64,
	/// The second argument.
	pub arg2:   u64,
	/// The third argument.
	pub arg3:   u64,
	/// The fourth argument.
	pub arg4:   u64,
}

/// A resumption type.
///
/// If provided to [`CoreHandle::run_context()`], the resumption type
/// parameterizes the return of execution back to the context.
pub enum Resumption {
	/// Return from a system call.
	SystemCall(SystemCallResponse),
}

/// System call response data.
#[derive(Debug, Clone, Copy)]
pub struct SystemCallResponse {
	/// The error code.
	pub error: oro::syscall::Error,
	/// The return value.
	pub ret:   u64,
}
