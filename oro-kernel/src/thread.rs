//! Thread management types and functions.

use crate::{instance::Instance, registry::Handle};
use oro_mem::mapper::AddressSpace;

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
pub struct Thread<AddrSpace: AddressSpace> {
	/// The thread's ID.
	pub(crate) id:       usize,
	/// The module instance to which this thread belongs.
	pub(crate) instance: Handle<Instance>,
	/// The thread's address space handle.
	pub(crate) space:    AddrSpace::UserHandle,
}

impl<AddrSpace: AddressSpace> Thread<AddrSpace> {
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
	pub fn instance(&self) -> Handle<Instance> {
		self.instance.clone()
	}

	/// Returns the thread's address space handle.
	#[must_use]
	pub fn space(&self) -> &AddrSpace::UserHandle {
		&self.space
	}
}
