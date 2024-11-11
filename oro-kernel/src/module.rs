//! Implements Oro module instances in the kernel.

use oro_id::{Id, IdType};
use oro_macro::assert;
use oro_mem::{
	alloc::{
		sync::{Arc, Weak},
		vec::Vec,
	},
	mapper::{AddressSpace, MapError},
};
use oro_sync::{Lock, Mutex};

use crate::{AddrSpace, Arch, Kernel, UserHandle, instance::Instance};

/// A singular executable module.
///
/// Modules are effectively executables in the Oro ecosystem,
/// representing an instanceable unit of code and data that can be
/// mounted onto a ring as an instance.
///
/// After creating a module, its [`Module::mapper()`] handle can be used
/// to populate the module with the executable code and data, which is then
/// used to create instances of the module (followed by a call to
/// [`crate::Arch::make_instance_unique()`]).
pub struct Module<A: Arch> {
	/// The resource ID.
	id: u64,
	/// The module ID. Provided by the ring spawner and used
	/// to refer to the module during module loading.
	module_id: Id<{ IdType::Module }>,
	/// The list of instances spawned from this module.
	pub(super) instances: Vec<Weak<Mutex<Instance<A>>>>,
	/// The module's address space mapper handle.
	///
	/// Only uninitialized if the module is in the process of being freed.
	pub(super) mapper: UserHandle<A>,
	/// A list of entry points for the module.
	///
	/// When modules are spawned as instances on a ring, each of the
	/// given entry points are spawned as threads.
	pub(super) entry_points: Vec<usize>,
}

impl<A: Arch> Module<A> {
	/// Creates a new module.
	pub fn new(module_id: Id<{ IdType::Module }>) -> Result<Arc<Mutex<Self>>, MapError> {
		let id = Kernel::<A>::get().state().allocate_id();

		let mapper = AddrSpace::<A>::new_user_space_empty().ok_or(MapError::OutOfMemory)?;

		let r = Arc::new(Mutex::new(Self {
			id,
			module_id,
			instances: Vec::new(),
			mapper,
			entry_points: Vec::new(),
		}));

		Kernel::<A>::get()
			.state()
			.modules
			.lock()
			.push(Arc::downgrade(&r));

		Ok(r)
	}

	/// Returns the instance ID.
	#[must_use]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// Returns the module ID.
	#[must_use]
	pub fn module_id(&self) -> &Id<{ IdType::Module }> {
		&self.module_id
	}

	/// Returns a list of weak handles to instances spawned from this module.
	pub fn instances(&self) -> &[Weak<Mutex<Instance<A>>>] {
		&self.instances
	}

	/// Returns the module's user address space mapper handle.
	///
	/// **All mappings created with this handle are shared between all instances of this module.**
	/// Thus, this handle should only be used for read-only mappings, or mappings that are RW
	/// and can either be duplicated or handled as COW (copy-on-write) by the architecture
	/// via the [`crate::Arch::make_instance_unique`] method.
	pub fn mapper(&self) -> &UserHandle<A> {
		&self.mapper
	}

	/// Adds an entry point to the module.
	///
	/// **IMPORTANT:** Calling this method with the same entry point multiple times will result in
	/// multiple threads being spawned for the same entry point. This may or may not be the desired
	/// behavior, depending on the use case.
	pub fn add_entry_point(&mut self, entry_point: usize) {
		self.entry_points.push(entry_point);
	}
}

impl<A: Arch> Drop for Module<A> {
	fn drop(&mut self) {
		// NOTE(qix-): Do not call assoiated methods on `self` within this method.

		// Make sure all instances have been properly destroyed prior
		// to dropping the module.
		for instance in &self.instances {
			if let Some(instance) = instance.upgrade() {
				panic!(
					"module dropped with active instance(s); first found is {:?}",
					instance.lock().id()
				);
			}
		}

		// Statically ensure that handles do not have drop semantics.
		// Otherwise, the following `unsafe` block would be unsound.
		assert::no_drop::<UserHandle<A>>();

		// SAFETY: We don't use the mapper after this point, so it's safe to zero it and take it.
		// SAFETY: Further, we've ensured that the mapper does not have drop semantics, so no
		// SAFETY: additional code is executed on the zeroed mapper handle.
		let mapper = core::mem::replace(&mut self.mapper, unsafe { core::mem::zeroed() });

		// Reclaim all pages from the module's address space.
		AddrSpace::<A>::free_user_space_deep(mapper);
	}
}
