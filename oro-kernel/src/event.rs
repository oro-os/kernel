//! Context switching events and resumption types.

/// A preemption event.
///
/// Passed to [`crate::Kernel::handle_event()`] to indicate the reason for the context switch
/// back to the kernel.
#[derive(Debug)]
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
	/// Interrupt number
	Interrupt(u64),
}

/// A page fault preemption event.
///
/// Returned by [`PreemptionEvent::PageFault`] to indicate the reason for the page fault.
#[derive(Debug)]
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
#[derive(Debug)]
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
/// If provided to [`crate::arch::CoreHandle::run_context()`], the resumption type
/// parameterizes the return of execution back to the context.
pub enum Resumption {
	/// Return from a system call.
	SystemCall(SystemCallResponse),
}

/// System call response data.
#[derive(Debug, Clone)]
pub struct SystemCallResponse {
	/// The error code.
	pub error: oro::syscall::Error,
	/// The return value.
	pub ret:   u64,
}
