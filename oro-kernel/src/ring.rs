//! Implements Oro rings in the kernel.

/// A singular ring.
///
/// Rings are collections of [`crate::module::ModuleInstance`]s.
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
pub struct Ring {
	/// The ring ID.
	///
	/// This is unique for each ring, but can be re-used if rings are destroyed.
	/// It is the offset of the arena slot into the arena pool.
	pub id:        usize,
	/// The parent ring ID.
	pub parent_id: usize,
}
