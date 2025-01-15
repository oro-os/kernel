//! Thread management types and functions.

use oro_mem::{
	alloc::sync::Arc,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace as _, MapError, UnmapError},
	pfa::Alloc,
};
use oro_sync::{Lock, ReentrantMutex};

use crate::{
	AddressSpace, Kernel, UserHandle,
	arch::{Arch, ThreadHandle},
	instance::Instance,
	interface::{SystemCallRequest, SystemCallResponse},
};

/// A thread's state.
#[derive(Debug, Clone, Default)]
#[expect(dead_code)]
enum State {
	/// The thread is not allocated to any core.
	#[default]
	Unallocated,
	/// The thread is stopped.
	Stopped,
	/// The thread is paused on the given core, awaiting a new time slice.
	Paused(u32),
	/// The thread is running on the given core.
	Running(u32),
	/// The thread invoked a system call, which is blocked and awaiting
	/// a response.
	PausedSystemCall(SystemCallRequest),
	/// The thread invoked a system call that has be responded to.
	///
	/// The next time it's scheduled, it will consume the handle
	/// and respond to the system call.
	RespondingSystemCall(SystemCallResponse),
	/// The thread is terminated.
	Terminated,
}

/// A singular system thread.
///
/// Threads are the primary unit of 'execution' in the
/// Oro kernel. They are scheduled by the kernel,
/// owned by a single core's [`crate::Kernel`] instance's
/// scheduler at any given time.
///
/// Threads belong to module [`Instance`]s and, unlike
/// other OSes, are not nested (i.e. a thread does not
/// have a parent thread).
pub struct Thread<A: Arch> {
	/// The resource ID.
	id:       u64,
	/// The module instance to which this thread belongs.
	instance: Arc<ReentrantMutex<Instance<A>>>,
	/// Architecture-specific thread state.
	handle:   A::ThreadHandle,
	/// The thread's state.
	state:    State,
}

impl<A: Arch> Thread<A> {
	/// Creates a new thread in the given module instance.
	#[expect(clippy::missing_panics_doc)]
	pub fn new(
		instance: &Arc<ReentrantMutex<Instance<A>>>,
		entry_point: usize,
	) -> Result<Arc<ReentrantMutex<Thread<A>>>, MapError> {
		let id = crate::id::allocate();

		// Pre-calculate the stack pointer.
		// TODO(qix-): If/when we support larger page sizes, this will need to be adjusted.
		let stack_ptr = AddressSpace::<A>::user_thread_stack().range().1 & !0xFFF;

		let mapper = AddressSpace::<A>::duplicate_user_space_shallow(instance.lock().mapper())
			.ok_or(MapError::OutOfMemory)?;

		let handle = A::ThreadHandle::new(mapper, stack_ptr, entry_point)?;

		// Allocate a thread stack.
		// XXX(qix-): This isn't very memory efficient, I just want it to be safe and correct
		// XXX(qix-): for now. At the moment, we allocate a blank userspace handle in order to
		// XXX(qix-): map in all of the stack pages, making sure all of the allocations work.
		// XXX(qix-): If they fail, then we can reclaim the entire address space back into the PFA
		// XXX(qix-): without having to worry about surgical unmapping of the larger, final
		// XXX(qix-): address space overlays (e.g. those coming from the ring, instance, module, etc).
		let thread_mapper =
			AddressSpace::<A>::new_user_space_empty().ok_or(MapError::OutOfMemory)?;

		let r = {
			let stack_segment = AddressSpace::<A>::user_thread_stack();
			let mut stack_ptr = stack_ptr;

			// Make sure the top guard page is unmapped.
			// This is more of a sanity check.
			match AddressSpace::<A>::user_thread_stack().unmap(&thread_mapper, stack_ptr) {
				Ok(phys) => {
					panic!(
						"empty user address space stack guard page was mapped to physical address \
						 {phys:#016X}"
					)
				}
				Err(UnmapError::NotMapped) => (),
				Err(e) => {
					panic!(
						"failed to assert unmap of empty user address space stack guard page: \
						 {e:?}"
					)
				}
			}

			// Map in the stack pages.
			// TODO(qix-): Allow this to be configurable
			for _ in 0..4 {
				stack_ptr -= 0x1000;
				let phys = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;
				stack_segment.map(&thread_mapper, stack_ptr, phys)?;
			}

			// Make sure the bottom guard page is unmapped.
			// This is more of a sanity check.
			stack_ptr -= 0x1000;
			match AddressSpace::<A>::user_thread_stack().unmap(&thread_mapper, stack_ptr) {
				Ok(phys) => {
					panic!(
						"empty user address space stack guard page was mapped to physical address \
						 {phys:#016X}"
					)
				}
				Err(UnmapError::NotMapped) => (),
				Err(e) => {
					panic!(
						"failed to assert unmap of empty user address space stack guard page: \
						 {e:?}"
					)
				}
			}

			Ok(())
		};

		if let Err(err) = r {
			AddressSpace::<A>::free_user_space_deep(thread_mapper);
			return Err(err);
		}

		// NOTE(qix-): Unwrap should never panic here barring a critical bug in the kernel.
		AddressSpace::<A>::user_thread_stack()
			.apply_user_space_shallow(handle.mapper(), &thread_mapper)
			.unwrap();

		AddressSpace::<A>::free_user_space_handle(thread_mapper);

		let r = Arc::new(ReentrantMutex::new(Self {
			id,
			instance: instance.clone(),
			handle,
			state: State::default(),
		}));

		instance.lock().threads.push(r.clone());

		Kernel::<A>::get()
			.state()
			.threads()
			.lock()
			.push(Arc::downgrade(&r));

		Ok(r)
	}

	/// Returns the thread's ID.
	#[must_use]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// Returns module instance handle to which this thread belongs.
	pub fn instance(&self) -> Arc<ReentrantMutex<Instance<A>>> {
		self.instance.clone()
	}

	/// Returns the thread's address space handle.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		self.handle.mapper()
	}

	/// Returns a refrence to the thread's architecture-specific handle.
	#[must_use]
	pub fn handle(&self) -> &A::ThreadHandle {
		&self.handle
	}

	/// Returns a mutable reference to the thread's architecture-specific handle.
	#[must_use]
	pub fn handle_mut(&mut self) -> &mut A::ThreadHandle {
		&mut self.handle
	}

	/// Attempts to schedule the thread on the given core.
	///
	/// # Safety
	/// The caller must **infallibly** consume any handles passed back
	/// in an `Ok` result, else they are forever lost, since this method
	/// advances the state machine and consumes the handle.
	pub unsafe fn try_schedule(&mut self, core_id: u32) -> Result<ScheduleAction, ScheduleError> {
		match &self.state {
			State::Terminated => Err(ScheduleError::Terminated),
			State::Running(core) => Err(ScheduleError::AlreadyRunning(*core)),
			State::Paused(core) => {
				if *core == core_id {
					self.state = State::Running(*core);
					Ok(ScheduleAction::Resume)
				} else {
					Err(ScheduleError::Paused(*core))
				}
			}
			State::Stopped => Err(ScheduleError::Stopped),
			State::PausedSystemCall(_) => Err(ScheduleError::AwaitingResponse),
			State::Unallocated => {
				self.handle.migrate();
				self.state = State::Running(core_id);
				Ok(ScheduleAction::Resume)
			}
			State::RespondingSystemCall(_) => {
				let State::RespondingSystemCall(response) =
					::core::mem::replace(&mut self.state, State::Running(core_id))
				else {
					unreachable!();
				};
				Ok(ScheduleAction::SystemCall(response))
			}
		}
	}

	/// Attempts to pause the thread on the given core.
	///
	/// The thread must already be running on the given core,
	/// else an error is returned.
	pub fn try_pause(&mut self, core_id: u32) -> Result<(), PauseError> {
		match &self.state {
			State::Terminated => Err(PauseError::Terminated),
			State::Running(core) => {
				if *core == core_id {
					self.state = State::Paused(core_id);
					Ok(())
				} else {
					Err(PauseError::WrongCore(*core))
				}
			}
			State::Paused(_)
			| State::Stopped
			| State::PausedSystemCall(_)
			| State::Unallocated
			| State::RespondingSystemCall(_) => Err(PauseError::NotRunning),
		}
	}
}

/// Error type for thread scheduling.
#[derive(Debug)]
pub enum ScheduleError {
	/// The thread is already running on the given core.
	AlreadyRunning(u32),
	/// The thread is terminated.
	Terminated,
	/// The thread needs an explicit response to an application request
	/// and cannot be scheduled normally.
	AwaitingResponse,
	/// The thread is paused on another core.
	Paused(u32),
	/// The thread is stopped.
	Stopped,
}

/// Error type for thread pausing (i.e. its timeslice has expired).
#[derive(Debug)]
pub enum PauseError {
	/// The thread was not running (either unallocated, paused, or stopped) - **but
	/// not terminated**.
	NotRunning,
	/// The thread is allocated to another core.
	WrongCore(u32),
	/// The thread is terminated.
	Terminated,
}

/// Action to take when scheduling a thread.
///
/// # Safety
/// Users of this enum MUST infallibly consume any handles passed back,
/// else they are forever lost.
pub enum ScheduleAction {
	/// The thread should be resumed normally.
	Resume,
	/// The thread needs to respond to a system call.
	SystemCall(SystemCallResponse),
}

impl<A: Arch> Drop for Thread<A> {
	fn drop(&mut self) {
		let old_state = core::mem::replace(&mut self.state, State::Terminated);

		// Sanity check; make sure the thread is not running on any scheduler,
		// as that indicates a bug in the kernel.
		assert!(!matches!(old_state, State::Running(_)));

		// SAFETY: Thread stack regions are specific to the thread and are not shared,
		// SAFETY: and thus safe to reclaim.
		unsafe {
			AddressSpace::<A>::user_thread_stack().unmap_all_and_reclaim(self.mapper());
		}
	}
}
