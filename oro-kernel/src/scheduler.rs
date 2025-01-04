//! Houses types, traits and functionality for the Oro kernel scheduler.

use oro_mem::alloc::sync::Arc;
use oro_sync::{Lock, Mutex};

use crate::{
	Kernel,
	arch::{Arch, CoreHandle},
	thread::Thread,
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
	current:    Option<Arc<Mutex<Thread<A>>>>,
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
	pub fn current_thread(&self) -> Option<Arc<Mutex<Thread<A>>>> {
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
	unsafe fn pick_user_thread(&mut self) -> Option<Arc<Mutex<Thread<A>>>> {
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

				if t.try_schedule(self.kernel.id()).is_ok() {
					// Select it for execution.
					drop(t);
					return Some(thread);
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
	pub unsafe fn event_idle(&mut self) -> Option<Arc<Mutex<Thread<A>>>> {
		let result = self.pick_user_thread();
		self.kernel.handle().schedule_timer(1000);
		result
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
	pub unsafe fn event_timer_expired(&mut self) -> Option<Arc<Mutex<Thread<A>>>> {
		let result = self.pick_user_thread();
		self.kernel.handle().schedule_timer(1000);
		result
	}
}
