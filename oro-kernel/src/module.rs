//! Implements Oro module instances in the kernel.

use oro_macro::assert;
use oro_mem::{
	alloc::vec::Vec,
	mapper::{AddressSpace as _, MapError},
};

use crate::{AddressSpace, UserHandle, arch::Arch, tab::Tab};

/// A singular executable module.
///
/// Modules are effectively executables in the Oro ecosystem,
/// representing an instanceable unit of code and data that can be
/// mounted onto a ring as an instance.
///
/// After creating a module, its [`Module::mapper()`] handle can be used
/// to populate the module with the executable code and data, which is then
/// used to create instances of the module.
pub struct Module<A: Arch> {
	/// The module's address space mapper handle.
	///
	/// Only uninitialized if the module is in the process of being freed.
	pub(super) mapper:       UserHandle<A>,
	/// A list of entry points for the module.
	///
	/// When modules are spawned as instances on a ring, each of the
	/// given entry points are spawned as threads.
	pub(super) entry_points: Vec<usize>,
}

impl<A: Arch> Module<A> {
	/// Creates a new module.
	pub fn new() -> Result<Tab<Self>, MapError> {
		let mapper = AddressSpace::<A>::new_user_space_empty().ok_or(MapError::OutOfMemory)?;

		crate::tab::get()
			.add(Self {
				mapper,
				entry_points: Vec::new(),
			})
			.ok_or(MapError::OutOfMemory)
	}

	/// Returns the module's user address space mapper handle.
	///
	/// **All mappings created with this handle are shared between all instances of this module.**
	/// Thus, this handle should only be used for read-only mappings, or mappings that are RW
	/// and can either be duplicated or handled as COW (copy-on-write) by the architecture.
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
		// NOTE(qix-): Do not call associated methods on `self` within this method.

		// Statically ensure that handles do not have drop semantics.
		// Otherwise, the following `unsafe` block would be unsound.
		assert::no_drop::<UserHandle<A>>();

		// SAFETY: We don't use the mapper after this point, so it's safe to zero it and take it.
		// SAFETY: Further, we've ensured that the mapper does not have drop semantics, so no
		// SAFETY: additional code is executed on the zeroed mapper handle.
		let mapper = core::mem::replace(&mut self.mapper, unsafe { core::mem::zeroed() });

		// Reclaim all pages from the module's address space.
		AddressSpace::<A>::free_user_space_deep(mapper);
	}
}
