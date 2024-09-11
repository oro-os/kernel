//! Implements Oro module instances in the kernel.
#![expect(clippy::module_name_repetitions)]

use crate::id::{Id, IdType};

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
///
/// # Module IDs
/// Module IDs internally are just the offset of the module instance
/// in the arena pool, divided by the size of the arena slot. Put simply,
/// if you think of the arena pool as an array of module instances,
/// the module ID is the index of the module instance in that array.
///
/// Module instances, rings, and ports all have their own ID spaces.
/// This means that a module instance, ring, and port can all have the
/// same ID, and they will not conflict with each other.
///
/// In no case should a module's ID be used to indicate its functionality,
/// version, or other metadata. It is purely an internal identifier, and
/// should be treated as random. It is only valid for the lifetime of the
/// module instance, and should not be stored or used outside of that context.
/// They should be treated like process IDs in a traditional operating system.
pub struct ModuleInstance {
	/// The instance ID. This is unique for each module instance,
	/// but can be re-used if instances are destroyed.
	///
	/// It is the offset of the arena slot into the arena pool.
	pub id:        u32,
	/// The module ID from which this instance was spawned.
	pub module_id: Id<{ IdType::Module }>,
}
