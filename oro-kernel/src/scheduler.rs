//! Houses types, traits and functionality for the Oro kernel scheduler.

use oro_mem::alloc::sync::Arc;
use oro_sync::{Lock, ReentrantMutex};

use crate::{
	Kernel,
	arch::{Arch, CoreHandle},
	interface::{InFlightSystemCall, InterfaceResponse, SystemCallRequest, SystemCallResponse},
	registry::Registry,
	thread::{RunState, ScheduleAction, Thread},
};

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
	/// A reference to the kernel instance.
	kernel:     &'static Kernel<A>,
	/// The current thread, if there is one being executed.
	current:    Option<Arc<ReentrantMutex<Thread<A>>>>,
	/// The index of the next thread to execute.
	next_index: usize,
}

// XXX(qix-): Temporary workaround to make things compile
// XXX(qix-): prior to heap allocation and scheduler refactor.
unsafe impl<A: Arch> Send for Scheduler<A> {}
unsafe impl<A: Arch> Sync for Scheduler<A> {}

impl<A: Arch> Scheduler<A> {
	/// Creates a new scheduler instance.
	pub(crate) fn new(kernel: &'static Kernel<A>) -> Self {
		Self {
			kernel,
			current: None,
			next_index: 0,
		}
	}

	/// Returns a handle to the currently processing thread.
	#[must_use]
	pub fn current_thread(&self) -> Option<Arc<ReentrantMutex<Thread<A>>>> {
		self.current.clone()
	}

	/// Selects a new thread to run.
	///
	/// This is one of the more expensive operations in the scheduler
	/// relatively speaking, so it should only be called once it's been
	/// determined that a new thread should be run.
	///
	/// It does NOT schedule kernel threads, only user threads.
	/// Kernel threads must be scheduled by the caller if needed.
	///
	/// Performs thread migration if the selected thread is assigned
	/// to our core but is not currently running on it.
	///
	/// Returns None if no user thread is available to run.
	///
	/// # Safety
	/// Interrupts MUST be disabled before calling this function.
	#[must_use]
	unsafe fn pick_user_thread(
		&mut self,
	) -> Option<(Arc<ReentrantMutex<Thread<A>>>, ScheduleAction)> {
		if let Some(thread) = self.current.take() {
			thread
				.lock()
				.try_pause(self.kernel.id())
				.expect("thread pause failed");
		}

		// XXX(qix-): This is a terrible design but gets the job done for now.
		// XXX(qix-): Every single core will be competing for a list of the same threads
		// XXX(qix-): until a thread migration system is implemented.
		let thread_list = self.kernel.state().threads().lock();

		while self.next_index < thread_list.len() {
			let thread = &thread_list[self.next_index];
			self.next_index += 1;

			if let Some(thread) = thread.upgrade() {
				let mut t = thread.lock();

				if let Ok(action) = t.try_schedule(self.kernel.id()) {
					// Select it for execution.
					drop(t);
					self.current = Some(thread.clone());
					return Some((thread, action));
				}
			}
		}

		drop(thread_list);

		self.next_index = 0;
		None
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
	/// It does _not_ need to be called with the original kernel
	/// stack in place, but _must_ run in the supervisor's context
	/// (including any permissions levels relevant for supervisory
	/// instructions to execute without access faults, as well as
	/// the kernel memory map being intact).
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.** At no point
	/// can other scheduler methods be invoked while this function
	/// is running.
	#[must_use]
	pub unsafe fn event_idle(&mut self) -> Switch<A> {
		let coming_from_user = self.current.as_ref().map(|t| t.lock().id());
		let switch = Switch::from_schedule_action(self.pick_user_thread(), coming_from_user);
		self.kernel.handle().schedule_timer(1000);
		switch
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
	/// It does _not_ need to be called with the original kernel
	/// stack in place, but _must_ run in the supervisor's context
	/// (including any permissions levels relevant for supervisory
	/// instructions to execute without access faults, as well as
	/// the kernel memory map being intact).
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.** At no point
	/// can other scheduler methods be invoked while this function
	/// is running.
	#[must_use]
	pub unsafe fn event_timer_expired(&mut self) -> Switch<A> {
		let coming_from_user = self.current.as_ref().map(|t| t.lock().id());
		let switch = Switch::from_schedule_action(self.pick_user_thread(), coming_from_user);
		self.kernel.handle().schedule_timer(1000);
		switch
	}

	/// Indicates to the kernel that a system call has been invoked
	/// by the currently running thread.
	///
	/// This function is called by the architecture-specific system
	/// call handler to indicate that a system call has been invoked
	/// by the currently running thread.
	///
	/// Either the system call will be processed and the thread will
	/// be resumed, or the thread will be paused and the system call
	/// will be processed asynchronously.
	///
	/// Similar to [`Self::event_timer_expired`], this function returns
	/// either a userspace handle to switch to and continue executing,
	/// or `None` if the architecture should enter a low-power / wait
	/// state until an interrupt or event occurs.
	///
	/// # Interrupt Safety
	/// This function is safe to call from an interrupt context,
	/// though it is _not_ explicitly required to be called from
	/// such a context.
	///
	/// It does _not_ need to be called with the original kernel
	/// stack in place, but _must_ run in the supervisor's context
	/// (including any permissions levels relevant for supervisory
	/// instructions to execute without access faults, as well as
	/// the kernel memory map being intact).
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.** At no point
	/// can other scheduler methods be invoked while this function
	/// is running.
	///
	/// # Panics
	/// Will panic if the kernel ever gets into an invalid state
	/// (indicating a bug in the scheduler logic / thread state machine).
	#[must_use]
	pub unsafe fn event_system_call(&mut self, request: &SystemCallRequest) -> Switch<A> {
		let coming_from_user = if let Some(thread) = self.current.take() {
			let mut t = thread.lock();

			let response = {
				if let Some(instance) = t.instance().lock().ring().upgrade() {
					let instance_lock = instance.lock();
					let registry = instance_lock.registry();
					let mut registry_lock = registry.lock();
					registry_lock.dispatch(&thread, request)
				} else {
					// The instance is gone; we can't do anything.
					// This is a critical error.
					panic!(
						"instance is still running but has no ring; kernel is in an invalid state"
					);
				}
			};

			// If the thread was stopped or terminated by the syscall, we need to
			// handle it specially.
			match (t.run_state(), response) {
				(RunState::Running, InterfaceResponse::Immediate(response)) => {
					drop(t);
					self.current = Some(thread.clone());
					return Switch::UserResume(thread, Some(response));
				}
				(RunState::Stopped, InterfaceResponse::Immediate(response)) => {
					let id = t.id();
					let (sub, handle) = InFlightSystemCall::new();
					t.await_system_call_response(self.kernel.id(), handle);
					drop(t);
					sub.submit(response);
					Some(id)
				}
				(RunState::Terminated, _) => {
					let id = t.id();
					drop(t);
					Some(id)
				}
				(RunState::Running | RunState::Stopped, InterfaceResponse::Pending(handle)) => {
					let id = t.id();
					t.await_system_call_response(self.kernel.id(), handle);
					drop(t);
					Some(id)
				}
			}
		} else {
			None
		};

		let switch = Switch::from_schedule_action(self.pick_user_thread(), coming_from_user);
		self.kernel.handle().schedule_timer(1000);
		switch
	}
}

/// Indicates the type of context switch to be taken by an event caller
/// (typically, the architecture).
///
/// Guaranteed to be correct state transitions (e.g.
/// will never return "kernel to user" when the current
/// run context is a user thread).
#[derive(Clone)]
pub enum Switch<A: Arch> {
	/// Coming from a user thread, return to kernel execution.
	UserToKernel,
	/// Coming from kernel execution, return to the given user thread.
	///
	/// If the system call handle is not `None`, the thread had invoked
	/// a system call and is awaiting a response.
	KernelToUser(Arc<ReentrantMutex<Thread<A>>>, Option<SystemCallResponse>),
	/// Coming from kernel execution, return back to the kernel.
	KernelResume,
	/// Coming from a user thread, return to the same user thread.
	///
	/// Thread handle is guaranteed to be the same as the one that
	/// was running before the context switch.
	///
	/// If no additional optimizations can be made in this case,
	/// treated exactly the same as [`Self::UserToUser`].
	///
	/// If the system call handle is not `None`, the thread had invoked
	/// a system call and is awaiting a response.
	UserResume(Arc<ReentrantMutex<Thread<A>>>, Option<SystemCallResponse>),
	/// Coming from a user thread, return to the given (different) user thread.
	///
	/// If the system call handle is not `None`, the thread had invoked
	/// a system call and is awaiting a response.
	UserToUser(Arc<ReentrantMutex<Thread<A>>>, Option<SystemCallResponse>),
}

impl<A: Arch> Switch<A> {
	/// Converts a schedule action and optional previous user thread ID
	/// into a switch type.
	#[must_use]
	fn from_schedule_action(
		action: Option<(Arc<ReentrantMutex<Thread<A>>>, ScheduleAction)>,
		coming_from_user: Option<u64>,
	) -> Self {
		match (action, coming_from_user) {
			(Some((thread, ScheduleAction::Resume)), None) => Switch::KernelToUser(thread, None),
			(Some((thread, ScheduleAction::Resume)), Some(old_id)) => {
				if thread.lock().id() == old_id {
					Switch::UserResume(thread, None)
				} else {
					Switch::UserToUser(thread, None)
				}
			}
			(Some((thread, ScheduleAction::SystemCall(syscall_res))), None) => {
				Switch::KernelToUser(thread, Some(syscall_res))
			}
			(Some((thread, ScheduleAction::SystemCall(syscall_res))), Some(old_id)) => {
				if thread.lock().id() == old_id {
					Switch::UserResume(thread, Some(syscall_res))
				} else {
					Switch::UserToUser(thread, Some(syscall_res))
				}
			}
			(None, None) => Switch::KernelResume,
			(None, Some(_)) => Switch::UserToKernel,
		}
	}
}
