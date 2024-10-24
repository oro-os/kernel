//! Implements Oro rings in the kernel.

use crate::{
	Arch,
	instance::Instance,
	registry::{Handle, List},
};

/// A singular ring.
///
/// Rings are collections of [`crate::instance::Instance`]s.
/// They also form the primary security boundary in the Oro ecosystem.
///
/// Module instances are mounted onto rings, allowing the instances to
/// see all other instances on the ring, as well as child rings.
///
/// However, module instances on a ring cannot see 'sibling' or parent
/// rings, or anything on them, under any circumstance. This is enforced
/// by the kernel. The resources they have access to are limited to those
/// explicitly granted to them by the parent ring via [`crate::port::Port`]s.
///
/// Rings have exactly one parent ring, and can have any number of child
/// rings. The root ring is the only ring that has no parent ring, and is
/// spawned by the kernel itself. Any boot module instances put onto
/// the root ring are effectively at the highest privilege level of the
/// system, and can interact with the kernel directly. Child rings may
/// only do so if one of the root ring's module instances has granted
/// them such access via a port.
pub struct Ring<A: Arch> {
	/// The ring ID.
	///
	/// This is unique for each ring, but can be re-used if rings are destroyed.
	/// It is the offset of the arena slot into the arena pool.
	pub(crate) id:        usize,
	/// The parent ring [`Handle`]. `None` if this is the root ring.
	pub(crate) parent:    Option<Handle<Ring<A>>>,
	/// The module [`Instance`]s on the ring.
	pub(crate) instances: Handle<List<Instance<A>, A>>,
}

impl<A: Arch> Ring<A> {
	/// Returns the ring's ID.
	///
	/// # Safety
	/// **DO NOT USE THIS FUNCTION FOR ANYTHING SECURITY RELATED.**
	///
	/// IDs are re-used by registries when items are dropped, so
	/// multiple calls to an ID lookup function may return handles to
	/// different ring items as the IDs get recycled.
	///
	/// Only use this function for debugging or logging purposes, or
	/// for handing IDs to the user.
	#[must_use]
	pub unsafe fn id(&self) -> usize {
		self.id
	}

	/// Returns the ring's parent ring [`Handle`].
	///
	/// If the ring is the root ring, this function will return `None`.
	#[must_use]
	pub fn parent(&self) -> Option<Handle<Ring<A>>> {
		self.parent.clone()
	}

	/// Returns a handle to the list of instances on the ring.
	pub fn instances(&self) -> Handle<List<Instance<A>, A>> {
		self.instances.clone()
	}
}
