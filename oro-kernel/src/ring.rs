//! Implements Oro rings in the kernel.

use oro_mem::{
	alloc::{
		sync::{Arc, Weak},
		vec::Vec,
	},
	mapper::{AddressSegment, AddressSpace as _, MapError},
};
use oro_sync::{Lock, ReentrantMutex};

use crate::{
	AddressSpace, Kernel, UserHandle,
	arch::Arch,
	instance::Instance,
	registry::{RegistryView, RootRegistry},
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
#[non_exhaustive]
pub struct Ring<A: Arch> {
	/// The resource ID.
	id:        u64,
	/// The parent ring handle, or `None` if this is the root ring.
	parent:    Option<Weak<ReentrantMutex<Ring<A>>>>,
	/// The module [`Instance`]s on the ring.
	instances: Vec<Arc<ReentrantMutex<Instance<A>>>>,
	/// The ring's base mapper handle.
	mapper:    UserHandle<A>,
	/// The ring's child rings.
	children:  Vec<Arc<ReentrantMutex<Ring<A>>>>,
	/// The ring's registry.
	registry:  Arc<ReentrantMutex<RegistryView<RootRegistry>>>,
}

impl<A: Arch> Ring<A> {
	/// Creates a new ring.
	pub fn new(
		parent: &Arc<ReentrantMutex<Ring<A>>>,
	) -> Result<Arc<ReentrantMutex<Self>>, MapError> {
		let id = crate::id::allocate();

		let mapper = AddressSpace::<A>::new_user_space(&Kernel::<A>::get().mapper)
			.ok_or(MapError::OutOfMemory)?;

		AddressSpace::<A>::sysabi().provision_as_shared(&mapper)?;

		let r = Arc::new(ReentrantMutex::new(Self {
			id,
			parent: Some(Arc::downgrade(parent)),
			instances: Vec::new(),
			mapper,
			children: Vec::new(),
			registry: Arc::new(ReentrantMutex::new(RegistryView::new(
				crate::Kernel::<A>::get().state().registry().clone(),
			))),
		}));

		parent.lock().children.push(r.clone());

		Kernel::<A>::get()
			.state()
			.rings
			.lock()
			.push(Arc::downgrade(&r));

		Ok(r)
	}

	/// Creates a new root ring.
	///
	/// # Safety
	/// May only be called once over the entire lifetime of the kernel state.
	///
	/// Intended to be assigned to the kernel state's `root_ring` field immediately
	/// after creation.
	///
	/// Caller **must** push the ring onto the kernel state's `rings` list itself;
	/// this method **will not** do it for you.
	pub(crate) unsafe fn new_root(
		registry: Arc<ReentrantMutex<RootRegistry>>,
	) -> Result<Arc<ReentrantMutex<Self>>, MapError> {
		// NOTE(qix-): This method CANNOT call `Kernel::<A>::get()` because
		// NOTE(qix-): core-local kernels are not guaranteed to be initialized
		// NOTE(qix-): at this point in the kernel's lifetime.

		// NOTE(qix-): We'd normally use the kernel's cached mapper instead of
		// NOTE(qix-): getting the supervisor mapper directly (since it's slower
		// NOTE(qix-): and less "safe" to pull it from the registers) but at this
		// NOTE(qix-): point it's the only way to get the supervisor mapper and is,
		// NOTE(qix-): for all intents and purposes, safe to do so. It's not ideal
		// NOTE(qix-): and might get refactored in the future to be even more bulletproof.
		let mapper =
			AddressSpace::<A>::new_user_space(&AddressSpace::<A>::current_supervisor_space())
				.ok_or(MapError::OutOfMemory)?;

		AddressSpace::<A>::sysabi().provision_as_shared(&mapper)?;

		let r = Arc::new(ReentrantMutex::new(Self {
			id: 0,
			parent: None,
			instances: Vec::new(),
			mapper,
			children: Vec::new(),
			registry: Arc::new(ReentrantMutex::new(RegistryView::new(registry))),
		}));

		Ok(r)
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
	pub fn parent(&self) -> Option<Weak<ReentrantMutex<Ring<A>>>> {
		self.parent.clone()
	}

	/// Returns a slice of instances on the ring.
	#[must_use]
	pub fn instances(&self) -> &[Arc<ReentrantMutex<Instance<A>>>] {
		&self.instances
	}

	/// Returns a reference to the ring's mapper.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		&self.mapper
	}

	/// Returns a mutable reference to the instances vector.
	#[must_use]
	pub fn instances_mut(&mut self) -> &mut Vec<Arc<ReentrantMutex<Instance<A>>>> {
		&mut self.instances
	}

	/// Returns the ring's registry handle.
	#[must_use]
	pub fn registry(&self) -> &Arc<ReentrantMutex<RegistryView<RootRegistry>>> {
		&self.registry
	}
}
