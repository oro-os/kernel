//! Houses types, traits and functionality for the Oro kernel scheduler.

use oro_sync::Lock;

use crate::{
	Arch, Kernel,
	registry::{Handle, Item},
	thread::Thread,
};

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
pub trait Handler<A: Arch> {
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

	/// Migrates the given thread to this kernel core.
	///
	/// This function is called when a thread is assigned to
	/// this core but is not currently running on it.
	///
	/// It must either succeed, or panic (killing the kernel).
	fn migrate_thread(kernel: &Kernel<A>, thread: &mut Thread<A>);
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
	/// A reference to the kernel instance.
	kernel:         &'static Kernel<A>,
	/// The current thread being run, as a list item.
	///
	/// `None` if no thread is currently running.
	current_thread: Option<Handle<Item<Thread<A>, A>>>,
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
			current_thread: None,
		}
	}

	/// Returns a [`Handle`] of the currently
	/// running thread, if any.
	///
	/// This must always return `Some` if a user task
	/// has been scheduled.
	///
	/// # Safety
	/// **Interrupts or any other asynchronous events must be
	/// disabled before calling this function.**
	#[must_use]
	pub unsafe fn current_thread(&self) -> Option<Handle<Thread<A>>> {
		self.current_thread
			.as_ref()
			.map(|item| item.lock().handle().clone())
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
	unsafe fn pick_user_thread<H: Handler<A>>(&mut self) -> Option<Handle<Thread<A>>> {
		loop {
			let selected_thread_item = if let Some(current_thread) = self.current_thread.take() {
				let next_thread = {
					let current_thread_lock = current_thread.lock();
					let next = if current_thread_lock.in_list() {
						current_thread_lock.next()
					} else {
						// The current thread was removed from the threads list
						// (probably pending removal), so we start from the beginning.
						// This shouldn't happen often, as it's a recoverable "race condition".
						// Not ideal but it's not a critical issue.
						None
					};
					drop(current_thread_lock);
					next
				};

				// If we've reached the end of the list, force breaking back into the kernel.
				Some(next_thread?)
			} else {
				self.kernel.state().threads().lock().first()
			}?;

			self.current_thread = Some(selected_thread_item.clone());

			let selected_thread_item_lock = selected_thread_item.lock();
			let mut selected_thread_lock = selected_thread_item_lock.lock();

			let this_kernel_id = self.kernel.id();

			// Try to claim any orphan threads.
			if *selected_thread_lock.run_on_id.get_or_insert(this_kernel_id) == this_kernel_id {
				let needs_migration = selected_thread_lock
					.running_on_id
					.map_or_else(|| true, |id| id != this_kernel_id);

				if needs_migration {
					H::migrate_thread(self.kernel, &mut *selected_thread_lock);
					selected_thread_lock.running_on_id = Some(this_kernel_id);
				}

				let result = selected_thread_item_lock.handle().clone();

				drop(selected_thread_lock);
				drop(selected_thread_item_lock);

				return Some(result);
			}

			drop(selected_thread_lock);
			drop(selected_thread_item_lock);
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
	pub unsafe fn event_idle<H: Handler<A>>(&mut self, handler: &H) -> Option<Handle<Thread<A>>> {
		let result = self.pick_user_thread::<H>();
		handler.schedule_timer(1000);
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
	pub unsafe fn event_timer_expired<H: Handler<A>>(
		&mut self,
		handler: &H,
	) -> Option<Handle<Thread<A>>> {
		let result = self.pick_user_thread::<H>();
		handler.schedule_timer(1000);
		result
	}
}
