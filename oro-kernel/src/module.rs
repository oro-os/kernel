//! Implements Oro module instances in the kernel.

use oro_id::{Id, IdType};
use oro_mem::alloc::{sync::Arc, vec::Vec};
use oro_sync::Mutex;

use crate::{Arch, UserHandle, instance::Instance};

/// Module metadata.
pub struct Module<A: Arch> {
	/// The instance ID. This is unique for each module instance,
	/// but can be re-used if instances are destroyed.
	///
	/// It is the offset of the arena slot into the arena pool.
	///
	/// **DO NOT USE THIS ID FOR ANYTHING SECURITY RELATED.**
	pub(crate) id:        usize,
	/// The module ID. Provided by the ring spawner and used
	/// to refer to the module during module loading.
	pub(crate) module_id: Id<{ IdType::Module }>,
	/// The list of instances spawned from this module.
	pub(crate) instances: Vec<Arc<Mutex<Instance<A>>>>,
	/// The module's address space mapper handle.
	///
	/// Instance and thread address space handles are typically
	/// cloned from this.
	pub(crate) mapper:    UserHandle<A>,
}

impl<A: Arch> Module<A> {
	/// Returns the instance ID.
	#[must_use]
	pub fn id(&self) -> usize {
		self.id
	}

	/// Returns the module ID.
	#[must_use]
	pub fn module_id(&self) -> &Id<{ IdType::Module }> {
		&self.module_id
	}

	/// Returns the list of instances spawned from this module.
	pub fn instances(&self) -> &[Arc<Mutex<Instance<A>>>] {
		&self.instances
	}

	/// Returns the module's user address space mapper handle.
	pub fn mapper(&self) -> &UserHandle<A> {
		&self.mapper
	}
}
