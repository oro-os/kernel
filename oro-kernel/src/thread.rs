//! Thread management types and functions.

use core::mem::MaybeUninit;

use oro_mem::alloc::sync::Arc;
use oro_sync::Mutex;

use crate::{Arch, UserHandle, instance::Instance};

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
	/// The thread's ID.
	pub(crate) id: usize,
	/// The module instance to which this thread belongs.
	pub(crate) instance: Arc<Mutex<Instance<A>>>,
	/// The thread's address space handle.
	///
	/// This is typically cloned from the instance's
	/// userspace handle.
	///
	/// The mapper is valid and can be assumed initialized
	/// after the call to [`crate::KernelState::create_thread`]
	/// returns.
	pub(crate) mapper: MaybeUninit<UserHandle<A>>,
	/// Architecture-specific thread state.
	///
	/// Valid and can be assumed initialized (at least, in terms
	/// of `MaybeUninit`'s guarantees) after the call to
	/// [`crate::KernelState::create_thread`] returns.
	pub(crate) thread_state: MaybeUninit<A::ThreadState>,
	/// The kernel core ID this thread should run on.
	///
	/// None if this thread hasn't been claimed by any core
	/// (or the core has powered off and the thread should
	/// be migrated).
	pub(crate) run_on_id: Option<usize>,
	/// The kernel core ID this thread is currently running on.
	///
	/// None if this thread is not currently running.
	pub(crate) running_on_id: Option<usize>,
}

impl<A: Arch> Thread<A> {
	/// Returns the thread's ID.
	#[must_use]
	pub fn id(&self) -> usize {
		self.id
	}

	/// Returns module instance [`Handle`] to which this thread belongs.
	pub fn instance(&self) -> Arc<Mutex<Instance<A>>> {
		self.instance.clone()
	}

	/// Returns the thread's address space handle.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		// SAFETY(qix-): Safe to call after the thread is created.
		// SAFETY(qix-): NOTE: This *does* mean it's UNSAFE to call this
		// SAFETY(qix-): NOTE: from within `Kernel::create_thread()`.
		unsafe { self.mapper.assume_init_ref() }
	}

	/// Returns the thread's architecture-specific state.
	#[must_use]
	pub fn thread_state(&self) -> &A::ThreadState {
		// SAFETY(qix-): Safe to call after the thread is created.
		// SAFETY(qix-): NOTE: This *does* mean it's UNSAFE to call this
		// SAFETY(qix-): NOTE: from within `Kernel::create_thread()`.
		unsafe { self.thread_state.assume_init_ref() }
	}

	/// Returns a mutable reference to the thread's architecture-specific state.
	#[must_use]
	pub fn thread_state_mut(&mut self) -> &mut A::ThreadState {
		// SAFETY(qix-): Safe to call after the thread is created.
		// SAFETY(qix-): NOTE: This *does* mean it's UNSAFE to call this
		// SAFETY(qix-): NOTE: from within `Kernel::create_thread()`.
		unsafe { self.thread_state.assume_init_mut() }
	}
}
