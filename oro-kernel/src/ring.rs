//! Implements Oro rings in the kernel.

use oro_mem::{
	alloc::{
		sync::{Arc, Weak},
		vec::Vec,
	},
	mapper::{AddressSegment, AddressSpace, MapError},
};
use oro_sync::{Lock, Mutex};

use crate::{AddrSpace, Arch, Kernel, UserHandle, instance::Instance};

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
#[non_exhaustive]
pub struct Ring<A: Arch> {
	/// The resource ID.
	id: u64,
	/// The parent ring [`Handle`]. `None` if this is the root ring.
	parent: Option<Weak<Mutex<Ring<A>>>>,
	/// The module [`Instance`]s on the ring.
	pub(super) instances: Vec<Arc<Mutex<Instance<A>>>>,
	/// The ring's base mapper handle.
	pub(super) mapper: UserHandle<A>,
	/// The ring's child rings.
	pub(super) children: Vec<Arc<Mutex<Ring<A>>>>,
}

impl<A: Arch> Ring<A> {
	/// Common constructor for creating a new ring with the given ID and optional parent.
	fn new_with(
		id: u64,
		parent: Option<&Arc<Mutex<Ring<A>>>>,
	) -> Result<Arc<Mutex<Self>>, MapError> {
		let mapper = AddrSpace::<A>::new_user_space(&Kernel::<A>::get().mapper)
			.ok_or(MapError::OutOfMemory)?;

		AddrSpace::<A>::sysabi().provision_as_shared(&mapper)?;

		let r = Arc::new(Mutex::new(Self {
			id,
			parent: parent.as_ref().map(|p| Arc::downgrade(p)),
			instances: Vec::new(),
			mapper,
			children: Vec::new(),
		}));

		if let Some(p) = parent.as_ref() {
			p.lock().children.push(r.clone());
		}
		Kernel::<A>::get()
			.state()
			.rings
			.lock()
			.push(Arc::downgrade(&r));

		Ok(r)
	}

	/// Creates a new ring.
	pub fn new(parent: &Arc<Mutex<Ring<A>>>) -> Result<Arc<Mutex<Self>>, MapError> {
		let id = Kernel::<A>::get().state().allocate_id();
		Self::new_with(id, Some(parent))
	}

	/// Creates a new root ring.
	///
	/// # Safety
	/// May only be called once over the entire lifetime of the kernel state.
	///
	/// Intended to be assigned to the kernel state's `root_ring` field immediately
	/// after creation.
	pub unsafe fn new_root() -> Result<Arc<Mutex<Self>>, MapError> {
		Self::new_with(0, None)
	}

	/// Returns the ring's ID.
	#[must_use]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// Returns the ring's parent ring weak handle.
	///
	/// If the ring is the root ring, this function will return `None`.
	#[must_use]
	pub fn parent(&self) -> Option<Weak<Mutex<Ring<A>>>> {
		self.parent.clone()
	}

	/// Returns a slice of instances on the ring.
	#[must_use]
	pub fn instances(&self) -> &[Arc<Mutex<Instance<A>>>] {
		&self.instances
	}
}
