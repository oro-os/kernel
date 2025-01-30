//! Thread management types and functions.

// TODO(qix-): As one might expect, thread state managemen here is a bit messy
// TODO(qix-): and error-prone. It could use an FSM to help smooth out the transitions,
// TODO(qix-): and to properly handle thread termination and cleanup. Further,
// TODO(qix-): the schedulers have a very inefficient way of checking for relevant
// TODO(qix-): work to schedule, and pull from a global (yes, really) thread _vector_,
// TODO(qix-): which obviously won't scale. If you're looking at this and see problems,
// TODO(qix-): I'm well aware of them. Trying to get things working first, then make
// TODO(qix-): them better.

use oro::{key, syscall::Error as SysError};
use oro_debug::dbg_warn;
use oro_macro::AsU64;
use oro_mem::{
	alloc::vec::Vec,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace as _, MapError, UnmapError},
	pfa::Alloc,
	phys::PhysAddr,
};

use crate::{
	AddressSpace, UserHandle,
	arch::{Arch, ThreadHandle},
	instance::Instance,
	port::PortEnd,
	ring::Ring,
	scheduler::PageFaultType,
	syscall::{InFlightState, InFlightSystemCall, InFlightSystemCallHandle, SystemCallResponse},
	tab::Tab,
	table::{Table, TypeTable},
	token::Token,
};

/// A thread's run state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsU64)]
#[repr(u64)]
pub enum RunState {
	/// The thread is terminated.
	Terminated = key!("term"),
	/// The thread is running.
	Running    = key!("run"),
	/// The thread is stopped.
	Stopped    = key!("stop"),
}

/// A thread's state during its executing (i.e. when its run state is [`RunState::Running`]).
///
/// Managed by the scheduler.
#[derive(Default)]
enum State {
	/// The thread is not allocated to any core.
	#[default]
	Unallocated,
	/// The thread is paused on the given core, awaiting a new time slice.
	Paused(u32),
	/// The thread is running on the given core.
	Running(u32),
	/// The thread invoked a system call, which is blocked and awaiting
	/// a response.
	PausedSystemCall(InFlightSystemCallHandle),
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
	/// The tab's ID. Stored here for internal stuff;
	/// exposed via [`Tab::id()`].
	id: u64,
	/// The module instance to which this thread belongs.
	instance: Tab<Instance<A>>,
	/// Architecture-specific thread state.
	handle: A::ThreadHandle,
	/// The thread's state (during running).
	state: State,
	/// The thread's run state, which dictates if the thread
	/// is to be scheduled or not.
	run_state: RunState,
	/// If `Some`, another thread has requested a `run_state`
	/// change and should be notified when it occurs.
	run_state_transition: Option<(RunState, InFlightSystemCall)>,
	/// Associated thread data.
	data: TypeTable,
	/// The virtual memory mappings of virtual addresses to tokens,
	/// covering the thread-local segments.
	///
	/// Values are `(token, page_id)` pairs.
	// TODO(qix-): This assumes a 4096 byte page size, and is also
	// TODO(qix-): not a very efficient data structure. It will be
	// TODO(qix-): replaced with a more efficient data structure
	// TODO(qix-): in the future.
	tls_token_vmap: Table<(Tab<Token>, usize)>,
	/// A list of active port endpoint page virtual bases for the current time slice.
	///
	/// Unmapped when the thread is paused.
	active_ports: Vec<usize>,
}

impl<A: Arch> Thread<A> {
	/// Creates a new thread in the given module instance.
	#[expect(clippy::missing_panics_doc)]
	pub fn new(
		instance: &Tab<Instance<A>>,
		entry_point: usize,
	) -> Result<Tab<Thread<A>>, MapError> {
		// Pre-calculate the stack pointer.
		// TODO(qix-): If/when we support larger page sizes, this will need to be adjusted.
		let stack_ptr = AddressSpace::<A>::user_thread_stack().range().1 & !0xFFF;

		let mapper = instance
			.with(|instance| AddressSpace::<A>::duplicate_user_space_shallow(instance.mapper()))
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

		// Create the thread.
		// We do this before we create the tab just in case we're OOM
		// and need to have the thread clean itself up.
		let this = Self {
			id: 0, // will be set later
			instance: instance.clone(),
			handle,
			state: State::default(),
			run_state: RunState::Running,
			run_state_transition: None,
			data: TypeTable::new(),
			tls_token_vmap: Table::new(),
			active_ports: Vec::new(),
		};

		let tab = crate::tab::get().add(this).ok_or(MapError::OutOfMemory)?;

		tab.with_mut(|t| t.id = tab.id());

		instance.with_mut(|instance| instance.threads.insert(tab.id(), tab.clone()));

		Ok(tab)
	}

	/// Returns module instance handle to which this thread belongs.
	#[inline]
	pub fn instance(&self) -> &Tab<Instance<A>> {
		&self.instance
	}

	/// Returns the thread's address space handle.
	#[must_use]
	#[inline]
	pub fn mapper(&self) -> &UserHandle<A> {
		self.handle.mapper()
	}

	/// Returns a refrence to the thread's architecture-specific handle.
	#[must_use]
	#[inline]
	pub fn handle(&self) -> &A::ThreadHandle {
		&self.handle
	}

	/// Returns a mutable reference to the thread's architecture-specific handle.
	#[must_use]
	#[inline]
	pub fn handle_mut(&mut self) -> &mut A::ThreadHandle {
		&mut self.handle
	}

	/// Returns the thread's [`RunState`].
	#[must_use]
	#[inline]
	pub fn run_state(&self) -> RunState {
		self.run_state
	}

	/// Attempts to schedule the thread on the given core.
	///
	/// # Safety
	/// The caller must **infallibly** consume any handles passed back
	/// in an `Ok` result, else they are forever lost, since this method
	/// advances the state machine and consumes the handle.
	pub unsafe fn try_schedule(&mut self, core_id: u32) -> Result<ScheduleAction, ScheduleError> {
		match &self.run_state {
			RunState::Terminated => Err(ScheduleError::Terminated),
			RunState::Running => {
				match &self.state {
					State::Running(core) => Err(ScheduleError::AlreadyRunning(*core)),
					State::Paused(core) => {
						if *core == core_id {
							self.state = State::Running(*core);
							Ok(ScheduleAction::Resume)
						} else {
							Err(ScheduleError::Paused(*core))
						}
					}
					State::PausedSystemCall(handle) => {
						let running_result = match handle.try_take_response() {
							Ok(None) => return Err(ScheduleError::AwaitingResponse),
							Ok(Some(response)) => response,
							Err(InFlightState::InterfaceCanceled) => {
								SystemCallResponse {
									error: SysError::Canceled,
									ret:   0,
								}
							}
							Err(_) => unreachable!(),
						};

						self.state = State::Running(core_id);

						Ok(ScheduleAction::SystemCall(running_result))
					}
					State::Unallocated => {
						self.handle.migrate();
						self.state = State::Running(core_id);
						Ok(ScheduleAction::Resume)
					}
				}
			}
			RunState::Stopped => Err(ScheduleError::Stopped),
		}
	}

	/// Returns the thread's owning ring.
	///
	/// **Note:** This will temporarily lock the thread's instance.
	pub fn ring(&self) -> Tab<Ring<A>> {
		self.instance.with(|instance| instance.ring().clone())
	}

	/// Attempts to pause the thread on the given core.
	///
	/// The thread must already be running on the given core,
	/// else an error is returned.
	///
	/// **NOTE:** This method is **NOT thread safe** and is used
	/// exclusively by the scheduler. **"Paused" does not mean
	/// "stopped"**; it means the thread is waiting for a new
	/// time slice.
	///
	/// Use [`Thread::transition_to`] to change the thread's state
	/// from a system call.
	pub fn try_pause(&mut self, core_id: u32) -> Result<(), PauseError> {
		match &self.state {
			State::Running(core) => {
				if *core == core_id {
					// For each active port endpoint, unmap it.
					for virt in &self.active_ports {
						let segment = AddressSpace::<A>::user_thread_local_data();
						if let Err(err) = segment.unmap(self.handle.mapper(), *virt) {
							dbg_warn!(
								"failed to unmap port endpoint at {virt:#016X} for thread \
								 {:#016X}: {err:?} (port may misbehave)",
								self.id
							);
						}
					}

					// Clear the active ports list
					self.active_ports.clear();

					// We're paused now.
					self.state = State::Paused(core_id);
					Ok(())
				} else {
					Err(PauseError::WrongCore(*core))
				}
			}
			State::Paused(_) | State::PausedSystemCall(_) | State::Unallocated => {
				Err(PauseError::NotRunning)
			}
		}
	}

	/// Attempts to change the thread's run state from a system call.
	///
	/// If the thread is the calling thread, the state is changed immediately.
	/// Otherwise, if the state cannot be changed immediately from another thread,
	/// a handle to the in-flight system call is returned.
	pub fn transition_to(
		&mut self,
		calling_thread_id: u64,
		new_state: RunState,
	) -> Result<Option<InFlightSystemCallHandle>, ChangeStateError> {
		debug_assert!(
			self.run_state != RunState::Terminated || (self.id != calling_thread_id),
			"a dead thread has somehow performed a syscall"
		);

		if self.run_state == new_state {
			return Ok(None);
		}

		if self.run_state == RunState::Terminated {
			return Err(ChangeStateError::Terminated);
		}

		if self.id == calling_thread_id {
			// SAFETY: We're the calling thread; we can always change state immediately.
			unsafe { self.set_run_state_unchecked(new_state) };
			Ok(None)
		} else {
			match (self.run_state, new_state) {
				(RunState::Running, new_state) => {
					match &self.state {
						State::Paused(_) | State::PausedSystemCall(_) | State::Unallocated => {
							// SAFETY: The thread is running but isn't executing a time slice,
							// SAFETY: so an immediate state transition is safe.
							unsafe {
								self.set_run_state_unchecked(new_state);
							}
							Ok(None)
						}
						State::Running(_) => {
							// If we are running, and a thread has already requested
							// a state change, we take precedence on the first one.
							if self.run_state_transition.is_some() {
								Err(ChangeStateError::Race)
							} else {
								// Otherwise, we request the state change and hand back
								// a handle to the caller.
								let (client, handle) = InFlightSystemCall::new();
								self.run_state_transition = Some((new_state, client));
								Ok(Some(handle))
							}
						}
					}
				}
				(RunState::Stopped, new_state) => {
					// SAFETY: We're stopped; resuming the thread has no side effects.
					unsafe {
						self.set_run_state_unchecked(new_state);
					}
					Ok(None)
				}
				(RunState::Terminated, _) => unreachable!(), // already handled
			}
		}
	}

	/// Internally sets the run state of the thread, cleaning up any resources
	/// upon termination.
	///
	/// # Safety
	/// This function **performs no error handling or checking** and will **blindly**
	/// set the run state to the given value. It is the caller's responsibility to
	/// ensure that the thread is in a valid state to be set to the given run state.
	unsafe fn set_run_state_unchecked(&mut self, new_run_state: RunState) {
		self.run_state = new_run_state;

		if new_run_state == RunState::Terminated {
			// Allow any in-flight system calls to be deemed canceled.
			self.state = State::Unallocated;

			// Remove the thread from the instance's thread table.
			self.instance
				.with_mut(|instance| instance.threads.remove(self.id));
		}
	}

	/// Terminates the thread immediately.
	///
	/// # Safety
	/// Caller must be ready to switch to a different thread.
	pub(crate) unsafe fn terminate(&mut self) {
		// TODO(qix-): This isn't very fleshed out for now; the bigger
		// TODO(qix-): goal is to get it working.
		self.set_run_state_unchecked(RunState::Terminated);
	}

	/// Tells the thread it's waiting for an in-flight system call response.
	///
	/// # Panics
	/// Panics if the thread is not running on the given core.
	pub fn await_system_call_response(&mut self, core_id: u32, handle: InFlightSystemCallHandle) {
		if let State::Running(core) = &self.state {
			assert_eq!(*core, core_id, "thread is running, but on a different core");
			self.state = State::PausedSystemCall(handle);
		} else {
			panic!("thread is not running on the given core");
		}
	}

	/// Spawns the thread. If the thread has already been spawned,
	/// this function does nothing.
	#[inline]
	pub fn spawn(this: Tab<Thread<A>>) {
		if this.with(|t| matches!(t.state, State::Unallocated)) {
			crate::Kernel::<A>::get()
				.state()
				.submit_unclaimed_thread(this);
		}
	}

	/// Tells the thread it's been deallocated by a scheduler.
	///
	/// # Safety
	/// The caller must ensure that the thread is not actively running on any core.
	pub(crate) unsafe fn deallocate(this: &Tab<Thread<A>>) {
		this.with_mut(|t| t.state = State::Unallocated);
	}

	/// Returns a reference to the thread's data table.
	#[inline]
	pub fn data(&self) -> &TypeTable {
		&self.data
	}

	/// Returns a mutable reference to the thread's data table.
	#[inline]
	pub fn data_mut(&mut self) -> &mut TypeTable {
		&mut self.data
	}

	/// Handles a fault by the thread.
	///
	/// **The `maybe_unaligned_virt` parameter is not guaranteed to be aligned
	/// to any page boundary.** In most cases, it is coming directly from a
	/// userspace application.
	///
	/// The `Token` must have been previously mapped via [`Thread::try_map_token_at`],
	/// or else this method will fail.
	///
	/// Will attempt to commit, change memory permissions, etc. as necessary, or fail.
	/// Upon failure, it's up to the caller to decide what to do.
	pub fn on_page_fault(
		&mut self,
		maybe_unaligned_virt: usize,
		_fault_type: PageFaultType,
	) -> Result<(), PageFaultError> {
		// TODO(qix-): We always assume a 4096 page boundary. This might change in the future.
		let virt = maybe_unaligned_virt & !0xFFF;

		let Some(virt_key) = u64::try_from(virt).ok() else {
			return Err(PageFaultError::BadVirt);
		};

		let (token, page_idx) = self
			.tls_token_vmap
			.get(virt_key)
			.cloned()
			.or_else(|| self.instance.with(|i| i.token_vmap.get(virt_key).cloned()))
			.ok_or(PageFaultError::BadVirt)?;

		token.with_mut(|t| {
			match t {
				Token::Normal(t) => {
					debug_assert!(page_idx < t.page_count());
					debug_assert!(
						t.page_size() == 4096,
						"page size != 4096 is not implemented"
					);
					let page_base = virt + (page_idx * t.page_size());
					let segment = AddressSpace::<A>::user_data();
					let phys = t
						.get_or_allocate(page_idx)
						.ok_or(PageFaultError::MapError(MapError::OutOfMemory))?;
					segment
						.map(self.handle.mapper(), page_base, phys.address_u64())
						.map_err(PageFaultError::MapError)?;
					Ok(())
				}
				Token::PortEndpoint(ep) => {
					debug_assert!(
						page_idx == 0,
						"slot map tokens must be exactly one page; attempt was made to commit \
						 page index >0"
					);
					let segment = AddressSpace::<A>::user_thread_local_data();

					segment
						.map(self.handle.mapper(), virt, ep.phys().address_u64())
						.map_err(PageFaultError::MapError)?;

					if ep.side() == PortEnd::Consumer {
						// `true` since we're calling it from the consumer thread.
						ep.advance::<A, true>();
						self.active_ports.push(virt);
					} else {
						// `false` since we're calling it from the producer thread.
						ep.advance::<A, false>();
					}

					Ok(())
				}
			}
		})
	}

	/// Maps (sets the base of and reserves) a [`Token`] into the instance's address space.
	///
	/// **This does not immediately map in any memory.** It only marks the range
	/// in the _internal kernel_ address space as reserved for the token, to be
	/// committed later (typically via a page fault calling [`Thread::on_page_fault`]).
	pub fn try_map_token_at(
		&mut self,
		token: &Tab<Token>,
		virt: usize,
	) -> Result<(), TokenMapError> {
		token.with(|t| {
			match t {
				Token::Normal(t) => {
					debug_assert!(
						t.page_size() == 4096,
						"page size != 4096 is not implemented"
					);
					debug_assert!(
						t.page_size().is_power_of_two(),
						"page size is not a power of 2"
					);

					if (virt & (t.page_size() - 1)) != 0 {
						return Err(TokenMapError::VirtNotAligned);
					}

					let segment = AddressSpace::<A>::user_data();

					// Make sure that none of the tokens exist in the vmap.
					self.instance.with_mut(|instance| {
						if !instance.tokens.contains(token.id()) {
							return Err(TokenMapError::BadToken);
						}

						for page_idx in 0..t.page_count() {
							let page_base = virt + (page_idx * t.page_size());

							if !virt_resides_within::<A>(&segment, page_base)
								|| !virt_resides_within::<A>(
									&segment,
									page_base + t.page_size() - 1,
								) {
								return Err(TokenMapError::VirtOutOfRange);
							}

							let virt = u64::try_from(page_base)
								.map_err(|_| TokenMapError::VirtOutOfRange)?;
							if instance.token_vmap.contains(virt) {
								return Err(TokenMapError::Conflict);
							}
						}

						// Everything's okay, map them into the vmap now.
						for page_idx in 0..t.page_count() {
							let page_base = virt + (page_idx * t.page_size());
							// NOTE(qix-): We can use `as` here since we already check the page base above.
							instance
								.token_vmap
								.insert(page_base as u64, (token.clone(), page_idx));
						}

						Ok(())
					})
				}
				Token::PortEndpoint(_) => {
					if (virt & 4095) != 0 {
						return Err(TokenMapError::VirtNotAligned);
					}

					let segment = AddressSpace::<A>::user_thread_local_data();

					if !self.instance.with(|i| i.tokens.contains(token.id())) {
						return Err(TokenMapError::BadToken);
					}

					// Make sure that no other token exists in the vmap.
					if !virt_resides_within::<A>(&segment, virt)
						|| !virt_resides_within::<A>(&segment, virt + 4095)
					{
						return Err(TokenMapError::VirtOutOfRange);
					}

					if self
						.tls_token_vmap
						.contains(u64::try_from(virt).map_err(|_| TokenMapError::VirtOutOfRange)?)
					{
						return Err(TokenMapError::Conflict);
					}

					// Everything's okay, map them into the vmap now.
					// NOTE(qix-): We can use `as` here since we already check the page base above.
					self.tls_token_vmap.insert(virt as u64, (token.clone(), 0));

					Ok(())
				}
			}
		})
	}

	/// Attempts to return a [`Token`] from the instance's token list.
	///
	/// Returns `None` if the token is not present.
	#[inline]
	#[must_use]
	pub fn token(&self, id: u64) -> Option<Tab<Token>> {
		self.instance.with(|i| i.tokens.get(id).cloned())
	}

	/// "Forgets" a [`Token`] from the instance's token list.
	///
	/// Returns the forgotten token, or `None` if the token is not present.
	#[inline]
	pub fn forget_token(&mut self, id: u64) -> Option<Tab<Token>> {
		self.instance.with_mut(|i| i.tokens.remove(id))
	}

	/// Inserts a [`Token`] into the instance's token list.
	///
	/// Returns the ID of the token.
	#[inline]
	pub fn insert_token(&mut self, token: Tab<Token>) -> u64 {
		self.instance.with_mut(|i| i.tokens.insert_tab(token))
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
		// SAFETY(qix-): Do NOT rely on `self.id` being valid here.
		// SAFETY(qix-): There's a chance the thread is dropped during construction,
		// SAFETY(qix-): before it's registered with the tab registry.

		let old_state = core::mem::replace(&mut self.state, State::Unallocated);

		// Sanity check; make sure the thread is not running on any scheduler,
		// as that indicates a bug in the kernel.
		assert!(!matches!(old_state, State::Running(_)));

		// Make sure it's cleaned itself up.
		unsafe {
			self.set_run_state_unchecked(RunState::Terminated);
		}

		// SAFETY: Thread stack regions are specific to the thread and are not shared,
		// SAFETY: and thus safe to reclaim.
		unsafe {
			AddressSpace::<A>::user_thread_stack().unmap_all_and_reclaim(self.mapper());
		}
	}
}

/// Error type returned when changing a thread's state.
#[repr(u64)]
pub enum ChangeStateError {
	/// The thread is terminated; it cannot be resumed or stopped.
	Terminated = 0,
	/// Another thread is already waiting for a response. Try again
	/// later.
	Race       = 1,
}

/// An error returned by [`Thread::on_page_fault`].
#[derive(Debug, Clone, Copy)]
pub enum PageFaultError {
	/// The virtual address was not found in the virtual map.
	BadVirt,
	/// Mapping a token failed.
	MapError(MapError),
}

/// An error returned by [`Thread::try_map_token_at`].
#[derive(Debug, Clone, Copy)]
pub enum TokenMapError {
	/// The virtual address is not aligned.
	VirtNotAligned,
	/// The virtual address is out of range for the
	/// thread's address space.
	VirtOutOfRange,
	/// The token was not found for the instance.
	BadToken,
	/// The mapping conflicts (overlaps) with another mapping.
	Conflict,
}

/// Checks if the given virtual address resides within the given address segment.
#[inline]
fn virt_resides_within<A: Arch>(
	segment: &<AddressSpace<A> as ::oro_mem::mapper::AddressSpace>::UserSegment,
	virt: usize,
) -> bool {
	// NOTE(qix-): Range is *inclusive*.
	let (first, last) = segment.range();
	virt >= first && virt <= last
}
