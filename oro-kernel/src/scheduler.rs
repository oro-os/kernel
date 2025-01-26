//! Houses types, traits and functionality for the Oro kernel scheduler.

use nolock::queues::{
	DequeueError,
	spsc::unbounded::{UnboundedReceiver as Receiver, UnboundedSender as Sender},
};
use oro_debug::dbg_warn;

use crate::{
	Kernel,
	arch::{Arch, CoreHandle},
	syscall::{InFlightSystemCall, InterfaceResponse, SystemCallRequest, SystemCallResponse},
	tab::Tab,
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
	kernel:    &'static Kernel<A>,
	/// The current thread, if there is one being executed.
	current:   Option<Tab<Thread<A>>>,
	/// The thread queue transfer side.
	thread_tx: Sender<Tab<Thread<A>>>,
	/// The thread queue receive side.
	thread_rx: Receiver<Tab<Thread<A>>>,
}

// XXX(qix-): Temporary workaround to make things compile
// XXX(qix-): prior to heap allocation and scheduler refactor.
unsafe impl<A: Arch> Send for Scheduler<A> {}
unsafe impl<A: Arch> Sync for Scheduler<A> {}

impl<A: Arch> Scheduler<A> {
	/// Creates a new scheduler instance.
	pub(crate) fn new(kernel: &'static Kernel<A>) -> Self {
		let (thread_rx, thread_tx) = nolock::queues::spsc::unbounded::queue();

		Self {
			kernel,
			current: None,
			thread_tx,
			thread_rx,
		}
	}

	/// Returns a handle to the currently processing thread.
	#[must_use]
	pub fn current_thread(&self) -> Option<Tab<Thread<A>>> {
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
	unsafe fn pick_user_thread(&mut self) -> Option<(Tab<Thread<A>>, ScheduleAction)> {
		if let Some(thread) = self.current.take() {
			thread
				.with_mut(|t| t.try_pause(self.kernel.id()))
				.expect("thread pause failed");

			if let Err((thread, err)) = self.thread_tx.enqueue(thread) {
				// We failed to schedule the thread.
				dbg_warn!(
					"scheduler {} failed to requeue thread {:#016X}: {err:?}",
					self.kernel.id(),
					thread.id()
				);
				self.kernel.state().submit_unclaimed_thread(thread);
			}
		}

		loop {
			let selected = match self.kernel.state().try_claim_thread() {
				Some(thread) => thread,
				None => {
					match self.thread_rx.try_dequeue() {
						Ok(thread) => thread,
						Err(DequeueError::Closed) => panic!("thread queue closed"),
						Err(DequeueError::Empty) => {
							return None;
						}
					}
				}
			};

			// Take action if needed, otherwise skip the thread; it'll be re-queued when
			// it needs to be.
			if let Ok(action) = selected.with_mut(|t| t.try_schedule(self.kernel.id())) {
				self.current = Some(selected.clone());
				return Some((selected, action));
			}
		}
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
		let coming_from_user = self.current.as_ref().map(|t| t.id());
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
		let coming_from_user = self.current.as_ref().map(|t| t.id());
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
	#[expect(clippy::missing_panics_doc)]
	#[must_use]
	pub unsafe fn event_system_call(&mut self, request: &SystemCallRequest) -> Switch<A> {
		let Some(thread) = self.current.take() else {
			panic!("event_system_call() called with no current thread");
		};

		let response = crate::syscall::dispatch(&thread, request);

		// If the thread was stopped or terminated by the syscall, we need to
		// handle it specially.
		match (thread.with(|t| t.run_state()), response) {
			(RunState::Running, InterfaceResponse::Immediate(response)) => {
				self.current = Some(thread.clone());
				// No timer scheduling necessary.
				return Switch::UserResume(thread.clone(), Some(response));
			}
			(RunState::Stopped, InterfaceResponse::Immediate(response)) => {
				let (sub, handle) = InFlightSystemCall::new();
				thread.with_mut(|t| t.await_system_call_response(self.kernel.id(), handle));
				sub.submit(response);
			}
			(RunState::Terminated, _) => {}
			(RunState::Running | RunState::Stopped, InterfaceResponse::Pending(handle)) => {
				thread.with_mut(|t| t.await_system_call_response(self.kernel.id(), handle));
			}
		}

		let switch = Switch::from_schedule_action(self.pick_user_thread(), Some(thread.id()));
		self.kernel.handle().schedule_timer(1000);
		switch
	}

	/// Indicates to the kernel that a page fault has occurred.
	///
	/// # Safety
	/// Calling architectures **must** treat "return back to same task"
	/// [`Switch`]es as to mean "retry the faulting memory operation". The
	/// kernel will NOT attempt to recover from fatal or unexpected page faults.
	///
	/// **Interrupts or any other asynchronous events must be disabled before
	/// calling this function.** At no point can other scheduler methods be
	/// invoked while this function is running.
	#[expect(clippy::missing_panics_doc)]
	#[must_use]
	pub unsafe fn event_page_fault(
		&mut self,
		fault_type: PageFaultType,
		vaddr: usize,
	) -> Switch<A> {
		// TODO(qix-): For now, we terminate the thread.
		// TODO(qix-): This will be fleshed out much more in the future.
		let Some(thread) = self.current.take() else {
			panic!("event_page_fault() called with no current thread");
		};

		let id = thread.id();

		let instance = thread.with(|t| t.instance().clone());

		let switch = if instance.with_mut(|i| i.try_commit_token_at(vaddr)).is_err() {
			unsafe {
				thread.with_mut(|t| t.terminate());
			}
			dbg_warn!(
				"thread {:#016X} terminated due to page fault: {fault_type:?} at {vaddr:016X}",
				id
			);
			Switch::from_schedule_action(self.pick_user_thread(), Some(id))
		} else {
			self.current = Some(thread.clone());
			Switch::UserResume(thread, None)
		};

		self.kernel.handle().schedule_timer(1000);
		switch
	}
}

impl<A: Arch> Drop for Scheduler<A> {
	fn drop(&mut self) {
		// Drain the thread queue.
		while let Ok(thread) = self.thread_rx.try_dequeue() {
			self.kernel.state().submit_unclaimed_thread(thread);
		}
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
	KernelToUser(Tab<Thread<A>>, Option<SystemCallResponse>),
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
	UserResume(Tab<Thread<A>>, Option<SystemCallResponse>),
	/// Coming from a user thread, return to the given (different) user thread.
	///
	/// If the system call handle is not `None`, the thread had invoked
	/// a system call and is awaiting a response.
	UserToUser(Tab<Thread<A>>, Option<SystemCallResponse>),
}

impl<A: Arch> Switch<A> {
	/// Converts a schedule action and optional previous user thread ID
	/// into a switch type.
	#[must_use]
	fn from_schedule_action(
		action: Option<(Tab<Thread<A>>, ScheduleAction)>,
		coming_from_user: Option<u64>,
	) -> Self {
		match (action, coming_from_user) {
			(Some((thread, ScheduleAction::Resume)), None) => Switch::KernelToUser(thread, None),
			(Some((thread, ScheduleAction::Resume)), Some(old_id)) => {
				if thread.id() == old_id {
					Switch::UserResume(thread, None)
				} else {
					Switch::UserToUser(thread, None)
				}
			}
			(Some((thread, ScheduleAction::SystemCall(syscall_res))), None) => {
				Switch::KernelToUser(thread, Some(syscall_res))
			}
			(Some((thread, ScheduleAction::SystemCall(syscall_res))), Some(old_id)) => {
				if thread.id() == old_id {
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

/// The type of page fault that is being handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultType {
	/// A read is being performed.
	Read,
	/// A write is being performed.
	Write,
	/// Instructions are being fetched for execution.
	Execute,
}
