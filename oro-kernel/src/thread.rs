//! Thread management types and functions.

use crate::{instance::Instance, registry::Handle, Arch, UserHandle};

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
	pub(crate) id:       usize,
	/// The module instance to which this thread belongs.
	pub(crate) instance: Handle<Instance<A>>,
	/// The thread's address space handle.
	///
	/// This is typically cloned from the instance's
	/// userspace handle.
	pub(crate) space:    UserHandle<A>,
}

impl<A: Arch> Thread<A> {
	/// Returns the thread's ID.
	///
	/// # Safety
	/// **DO NOT USE THIS FUNCTION FOR ANYTHING SECURITY RELATED.**
	///
	/// IDs are re-used by registries when items are dropped, so
	/// multiple calls to an ID lookup function may return handles to
	/// different thread items as the IDs get recycled.
	///
	/// Only use this function for debugging or logging purposes, or
	/// for handing IDs to the user.
	#[must_use]
	pub unsafe fn id(&self) -> usize {
		self.id
	}

	/// Returns module instance [`Handle`] to which this thread belongs.
	pub fn instance(&self) -> Handle<Instance<A>> {
		self.instance.clone()
	}

	/// Returns the thread's address space handle.
	#[must_use]
	pub fn space(&self) -> &UserHandle<A> {
		&self.space
	}
}
