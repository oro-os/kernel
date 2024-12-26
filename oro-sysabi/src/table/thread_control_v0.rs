//! The Thread Control table (version 0).
//!
//! This table operates on a thread entity and provides control over the thread's state.
//!
//! This includes, but is not limited to, termination and suspension.

use super::table_id;

/// The ID of the Thread Control table (version 0).
pub const ID: u64 = table_id!("THRDCTL\0");

/// The keys supported by the Thread Control table (version 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Key {
	/// The thread's entity ID.
	///
	/// This key is read-only.
	EntityId       = 0,

	/// The thread's running state.
	///
	/// This key is read-only.
	///
	/// See the [`ThreadRunState`] enum for possible values.
	RunState       = 1,

	/// Signals that the thread is to be terminated.
	///
	/// This key is read-write. Writing any value to this key
	/// will cause the thread to terminate.
	///
	/// Writing a zero value to this key is undefined behavior - it
	/// will either terminate the thread, return an error, or have no effect.
	Terminate      = 2,

	/// The thread's yield time, in microseconds.
	///
	/// The kernel will yield the thread and re-wake it
	/// after the specified time has elapsed.
	///
	/// This key is write-only. It is not updated based on
	/// the thread's remaining yield time, and is set back to 0
	/// after the kernel reads it.
	///
	/// Writing a zero value to this key is undefined behavior.
	/// It may yield the thread, return an error, or have no effect.
	///
	/// To yield the time slice but not sleep, write a value of 1.
	///
	/// Note that memory-mapped tables may not immediately pick up
	/// on changes to this key. In cases where this is a concern,
	/// a syscall is recommended.
	YieldTime      = 3,

	/// The thread priority hint. Lower values indicate higher priority.
	///
	/// This key is read-write.
	///
	/// The kernel may choose to min-cap the priority to a certain value,
	/// or to ignore the priority entirely. The exact behavior is
	/// implementation-defined.
	///
	/// The kernel's responsiveness to changes to this key is also implementation
	/// defined (though typically taking effect prior to the next time slice).
	/// In cases where thread priority changes are critical, a syscall is recommended
	/// followed by an immediate yield.
	Priority       = 4,

	/// The current thread's priority, as determined by the kernel.
	///
	/// This value may be the same as [`Key::Priority`], or it may be different
	/// (either higher or lower), but always reflects the kernel's current
	/// priority assignment for the thread.
	///
	/// This key is read-only.
	KernelPriority = 5,

	/// The thread's current time slice counter.
	///
	/// This counter is monotonically incremented by the kernel
	/// each time the thread is scheduled for execution, and is affected
	/// by preemption, yielding, and other factors.
	///
	/// This key is read-only.
	TimeSlice      = 6,

	/// A user-defined thread identifier (UDID).
	///
	/// This key is read-write. It defaults to the thread's entity ID.
	///
	/// The kernel does not enforce uniqueness of this value, nor does
	/// the kernel interpret this value in any way. It will blindly
	/// store and return the value as requested.
	Udid           = 7,

	/// The thread's number within the module instance.
	///
	/// This is a monotonically increasing counter that starts at 0
	/// for the first thread in the module instance and increments by 1
	/// for each subsequent thread that is created. Thread numbers
	/// are not reused.
	///
	/// This key is read-only.
	ThreadNumber   = 8,

	/// While non-zero, the thread is not rescheduled for execution.
	///
	/// This field is read-write.
	///
	/// While this key is non-zero, updates to other keys in this table
	/// may not be responded to by the kernel. This includes [`Key::Priority`]
	/// and [`Key::YieldTime`].
	///
	/// Notably, [`Key::Terminate`] is still honored even if the thread is stopped.
	Stopped        = 9,
}

/// Values for the [`Key::RunState`] key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
#[non_exhaustive]
pub enum ThreadRunState {
	/// Thread has been registered with the kernel but not yet instantiated nor
	/// executed.
	Pending     = 0,
	/// Thread is running.
	Running     = 1,
	/// Thread is in the process of terminating (its [`Key::Terminate`] flag
	/// was raised).
	Terminating = 2,
	/// The thread has been terminated.
	Terminated  = 3,
	/// The thread has been stopped. This is the equivalent of an infinity
	/// yield.
	Stopped     = 4,
}
