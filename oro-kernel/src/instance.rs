//! Module instance types and functionality.

use crate::{module::Module, registry::Handle, ring::Ring};

/// A singular module instance.
///
/// Modules are effectively executables in the Oro ecosystem,
/// loading similarly to processes in a traditional operating system.
/// By themselves, modules do not do anything - it is when they are
/// mounted onto a ring as an instance (hence "module instance")
/// that they are effectively spawned and executed.
///
/// The kernel does not keep modules in memory; only module instances.
///
/// Further, not every module instance comes from a discrete module;
/// on the root ring, the kernel mounts several built-in modules
/// as instances to interact with system resources at a very low level.
/// These are often referred to as "built-in modules" or "kernel modules".
/// Unlike e.g. Linux, kernel modules are not extensible nor can they be
/// added via user configuration; they are hard-coded into the kernel,
/// and are often architecture-specific.
///
/// Typically the bootloader will have some means by which to load modules
/// as instances onto the root ring, since without any additional application-
/// specific modules, the kernel is effectively useless (will do nothing on
/// boot). The preboot routine (that jumps to the kernel, see `oro_boot::boot_to_kernel()`)
/// provides a means for memory-mapped portable executables (PEs) to be loaded
/// onto the root ring as instances.
///
/// Those instances will have the highest privilege level, and will be able
/// to interact with the kernel directly via the built-in modules, and
/// from there can spawn additional rings and instances as needed to
/// bootstrap the rest of the system as they see fit.
pub struct Instance {
	/// The module instance ID.
	id:     usize,
	/// The module from which this instance was spawned.
	module: Handle<Module>,
	/// The ring on which this instance resides.
	ring:   Handle<Ring>,
}

impl Instance {
	/// Returns the instance ID.
	///
	/// # Safety
	/// **DO NOT USE THIS ID FOR ANYTHING SECURITY RELATED.**
	///
	/// IDs are recycled when instances are dropped from the registry,
	/// meaning functions that accept numeric IDs might return or work
	/// with [`Handle`]s that refer to unintended instances that have
	/// been inserted into the same slot.
	#[must_use]
	pub unsafe fn id(&self) -> usize {
		self.id
	}

	/// The [`Handle`] to the module from which this instance was spawned.
	pub fn module(&self) -> Handle<Module> {
		self.module.clone()
	}

	/// The [`Handle`] to the ring on which this instance resides.
	pub fn ring(&self) -> Handle<Ring> {
		self.ring.clone()
	}
}
