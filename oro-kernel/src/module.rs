//! Implements Oro module instances in the kernel.

use crate::id::{Id, IdType};

/// Module metadata.
pub struct Module {
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
}

impl Module {
	/// Returns the instance ID.
	///
	/// # Safety
	/// **DO NOT USE THIS ID FOR ANYTHING SECURITY RELATED.**
	///
	/// IDs are re-used by registries when items are dropped, so
	/// functions that take numeric IDs to return [`crate::registry::Handle`]s
	/// may return a new item unexpectedly if the old one was dropped
	/// and the slot was re-used.
	#[must_use]
	pub unsafe fn id(&self) -> usize {
		self.id
	}

	/// Returns the module ID.
	#[must_use]
	pub fn module_id(&self) -> &Id<{ IdType::Module }> {
		&self.module_id
	}
}
