//! Thread management types and functions.

use oro_mem::{
	alloc::sync::Arc,
	global_alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace as _, MapError, UnmapError},
	pfa::Alloc,
};
use oro_sync::{Lock, Mutex};

use crate::{
	AddressSpace, Kernel, UserHandle,
	arch::{Arch, ThreadHandle},
	instance::Instance,
};

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
	/// Architecture-specific thread state.
	pub handle: A::ThreadHandle,
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

		let r = Arc::new(Mutex::new(Self {
			id,
			instance: instance.clone(),
			handle,
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

	/// Returns module instance handle to which this thread belongs.
	pub fn instance(&self) -> Arc<Mutex<Instance<A>>> {
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

		// SAFETY: Thread stack regions are specific to the thread and are not shared,
		// SAFETY: and thus safe to reclaim.
		unsafe {
			AddressSpace::<A>::user_thread_stack().unmap_all_and_reclaim(self.mapper());
		}
	}
}
