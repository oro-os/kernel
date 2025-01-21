//! Implements Oro rings in the kernel.

use oro_mem::{
	alloc::vec::Vec,
	mapper::{AddressSegment, AddressSpace as _, MapError},
};

use crate::{
	AddressSpace, Kernel, UserHandle,
	arch::Arch,
	instance::Instance,
	interface::{Interface, RingInterface},
	tab::Tab,
	table::Table,
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
/// explicitly granted to them by the parent ring via [`crate::interface::Interface`]s.
///
/// Rings have exactly one parent ring, and can have any number of child
/// rings. The root ring is the only ring that has no parent ring, and is
/// spawned by the kernel itself. Any boot module instances put onto
/// the root ring are effectively at the highest privilege level of the
/// system, and can interact with the kernel directly. Child rings may
/// only do so if one of the root ring's module instances has granted
/// them such access via a port or interface.
#[non_exhaustive]
pub struct Ring<A: Arch> {
	/// The parent ring handle, or `None` if this is the root ring.
	parent: Option<Tab<Ring<A>>>,
	/// The module [`Instance`]s on the ring.
	instances: Vec<Tab<Instance<A>>>,
	/// The ring's base mapper handle.
	mapper: UserHandle<A>,
	/// The ring's child rings.
	children: Vec<Tab<Ring<A>>>,
	/// The interfaces exposed to the ring, grouped by type.
	interfaces_by_type: Table<Vec<Tab<RingInterface<A>>>>,
}

impl<A: Arch> Ring<A> {
	/// Creates a new ring.
	pub fn new(parent: &Tab<Ring<A>>) -> Result<Tab<Self>, MapError> {
		let mapper = AddressSpace::<A>::new_user_space(&Kernel::<A>::get().mapper)
			.ok_or(MapError::OutOfMemory)?;

		AddressSpace::<A>::sysabi().provision_as_shared(&mapper)?;

		let tab = crate::tab::get()
			.add(Self {
				parent: Some(parent.clone()),
				instances: Vec::new(),
				mapper,
				children: Vec::new(),
				interfaces_by_type: Table::new(),
			})
			.ok_or(MapError::OutOfMemory)?;

		parent.with_mut(|parent| parent.children.push(tab.clone()));

		Ok(tab)
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
	pub(crate) unsafe fn new_root() -> Result<Tab<Self>, MapError> {
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

		crate::tab::get()
			.add(Self {
				parent: None,
				instances: Vec::new(),
				mapper,
				children: Vec::new(),
				interfaces_by_type: Table::new(),
			})
			.ok_or(MapError::OutOfMemory)
	}

	/// Returns the ring's parent ring weak handle.
	///
	/// If the ring is the root ring, this function will return `None`.
	#[must_use]
	pub fn parent(&self) -> Option<&Tab<Ring<A>>> {
		self.parent.as_ref()
	}

	/// Returns a reference to the ring's mapper.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		&self.mapper
	}

	/// Returns a slice of instances on the ring.
	#[must_use]
	pub fn instances(&self) -> &[Tab<Instance<A>>] {
		&self.instances
	}

	/// Returns a mutable reference to the instances vector.
	#[must_use]
	pub fn instances_mut(&mut self) -> &mut Vec<Tab<Instance<A>>> {
		&mut self.instances
	}

	/// Returns a slice of interfaces exposed to the ring, grouped by type.
	#[must_use]
	pub fn interfaces_by_type(&self) -> &Table<Vec<Tab<RingInterface<A>>>> {
		&self.interfaces_by_type
	}

	/// Returns a mutable reference to the interfaces table, grouped by type.
	#[must_use]
	pub fn interfaces_by_type_mut(&mut self) -> &mut Table<Vec<Tab<RingInterface<A>>>> {
		&mut self.interfaces_by_type
	}

	/// Convience function for registering an interface with the global tab
	/// system as well as the ring.
	///
	/// Returns `None` if the addition to the tab registry failed.
	/// See [`crate::tab::GlobalTable::add`] for more information.
	pub fn register_interface(&mut self, iface: RingInterface<A>) -> Option<Tab<RingInterface<A>>> {
		let type_id = iface.type_id();
		let tab = crate::tab::get().add(iface)?;
		self.interfaces_by_type
			.get_or_insert_mut(type_id)
			.push(tab.clone());
		Some(tab)
	}
}
