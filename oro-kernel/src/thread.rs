//! Thread management types and functions.

use oro_macro::assert;
use oro_mem::{
	alloc::sync::Arc,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError, UnmapError},
	pfa::Alloc,
};
use oro_sync::{Lock, Mutex};

use crate::{AddrSpace, Arch, Kernel, UserHandle, instance::Instance};

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
	pub id: u64,
	/// The module instance to which this thread belongs.
	pub instance: Arc<Mutex<Instance<A>>>,
	/// The thread's address space handle.
	pub mapper: UserHandle<A>,
	/// Architecture-specific thread state.
	pub thread_state: A::ThreadState,
	/// The kernel core ID this thread should run on.
	///
	/// None if this thread hasn't been claimed by any core
	/// (or the core has powered off and the thread should
	/// be migrated).
	pub run_on_id: Option<usize>,
	/// The kernel core ID this thread is currently running on.
	///
	/// None if this thread is not currently running.
	pub running_on_id: Option<usize>,
}

impl<A: Arch> Thread<A> {
	/// Creates a new thread in the given module instance.
	#[expect(clippy::missing_panics_doc)]
	pub fn new(
		instance: &Arc<Mutex<Instance<A>>>,
		entry_point: usize,
	) -> Result<Arc<Mutex<Thread<A>>>, MapError> {
		let id = Kernel::<A>::get().state().allocate_id();

		// Allocate a thread stack.
		// XXX(qix-): This isn't very memory efficient, I just want it to be safe and correct
		// XXX(qix-): for now. At the moment, we allocate a blank userspace handle in order to
		// XXX(qix-): map in all of the stack pages, making sure all of the allocations work.
		// XXX(qix-): If they fail, then we can reclaim the entire address space back into the PFA
		// XXX(qix-): without having to worry about surgical unmapping of the larger, final
		// XXX(qix-): address space overlays (e.g. those coming from the ring, instance, module, etc).
		let thread_mapper = AddrSpace::<A>::new_user_space_empty().ok_or(MapError::OutOfMemory)?;

		let stack_ptr = {
			let stack_segment = AddrSpace::<A>::user_thread_stack();

			// TODO(qix-): If/when we support larger page sizes, this will need to be adjusted.
			let mut stack_ptr = stack_segment.range().1 & !0xFFF;

			// Make sure the top guard page is unmapped.
			// This is more of a sanity check.
			match AddrSpace::<A>::user_thread_stack().unmap(&thread_mapper, stack_ptr) {
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

			let final_stack_ptr = stack_ptr;

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
			match AddrSpace::<A>::user_thread_stack().unmap(&thread_mapper, stack_ptr) {
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

			Ok(final_stack_ptr)
		};

		let stack_ptr = match stack_ptr {
			Ok(p) => p,
			Err(err) => {
				AddrSpace::<A>::free_user_space_deep(thread_mapper);
				return Err(err);
			}
		};

		let mapper = match AddrSpace::<A>::duplicate_user_space_shallow(instance.lock().mapper())
			.ok_or(MapError::OutOfMemory)
		{
			Ok(m) => m,
			Err(e) => {
				AddrSpace::<A>::free_user_space_deep(thread_mapper);
				return Err(e);
			}
		};

		// NOTE(qix-): Unwrap should never panic here barring a critical bug in the kernel.
		AddrSpace::<A>::user_thread_stack()
			.apply_user_space_shallow(&mapper, &thread_mapper)
			.unwrap();

		AddrSpace::<A>::free_user_space_handle(thread_mapper);

		let mut thread_state = A::new_thread_state(stack_ptr, entry_point);

		if let Err(err) = A::initialize_thread_mappings(&mapper, &mut thread_state) {
			// TODO(qix-): Double check this is correct...
			// SAFETY: We just allocated this address space, so it should be safe to unmap its thread data.
			unsafe {
				AddrSpace::<A>::user_thread_stack().unmap_all_and_reclaim(&mapper);
			}
			AddrSpace::<A>::free_user_space_handle(mapper);
			return Err(err);
		}

		let r = Arc::new(Mutex::new(Self {
			id,
			instance: instance.clone(),
			mapper,
			thread_state,
			run_on_id: None,
			running_on_id: None,
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

	/// Returns module instance [`Handle`] to which this thread belongs.
	pub fn instance(&self) -> Arc<Mutex<Instance<A>>> {
		self.instance.clone()
	}

	/// Returns the thread's address space handle.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		&self.mapper
	}

	/// Returns the thread's architecture-specific state.
	#[must_use]
	pub fn thread_state(&self) -> &A::ThreadState {
		&self.thread_state
	}

	/// Returns a mutable reference to the thread's architecture-specific state.
	#[must_use]
	pub fn thread_state_mut(&mut self) -> &mut A::ThreadState {
		&mut self.thread_state
	}
}

impl<A: Arch> Drop for Thread<A> {
	fn drop(&mut self) {
		// Make sure that, for whatever reason, a scheduler doesn't try to
		// run this thread after it's been dropped. This isn't 100% correct
		// but is a good enough deterrent.
		//
		// It's important to do this BEFORE the `running_on_id` check, just for
		// making things every so slightly more bulletproof.
		//
		// XXX(qix-): Create a better mechanism for preventing dead-thread scheduling.
		self.run_on_id = Some(usize::MAX);

		// Sanity check; make sure the thread is not running on any scheduler,
		// as that indicates a bug in the kernel.
		assert!(self.running_on_id.is_none());

		A::reclaim_thread_mappings(&self.mapper, &mut self.thread_state);

		// SAFETY: Thread stack regions are specific to the thread and are not shared,
		// SAFETY: and thus safe to reclaim.
		unsafe {
			AddrSpace::<A>::user_thread_stack().unmap_all_and_reclaim(&self.mapper);
		}

		// Statically ensure that handles have no drop semantics. Otherwise, the following
		// unsafe block would be unsound.
		assert::no_drop::<UserHandle<A>>();

		// SAFETY: We are about to destruct the userspace handle and have checked
		// SAFETY: that no drop code is executed, so replacing it with a zeroed
		// SAFETY: handle has no effect.
		let mapper = core::mem::replace(&mut self.mapper, unsafe { core::mem::zeroed() });

		AddrSpace::<A>::free_user_space_handle(mapper);
	}
}
