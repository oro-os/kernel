//! Module instance types and functionality.

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
	arch::{Arch, InstanceHandle},
	module::Module,
	port::Port,
	ring::Ring,
	thread::Thread,
};

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
#[non_exhaustive]
pub struct Instance<A: Arch> {
	/// The module instance ID.
	id: u64,
	/// The module from which this instance was spawned.
	///
	/// Strong reference to prevent the module from being
	/// deallocated while the instance is still alive, which would
	/// otherwise reclaim the executable memory pages and wreak havoc.
	module: Arc<ReentrantMutex<Module<A>>>,
	/// The ring on which this instance resides.
	ring: Weak<ReentrantMutex<Ring<A>>>,
	/// The thread list for the instance.
	pub(super) threads: Vec<Arc<ReentrantMutex<Thread<A>>>>,
	/// The port list for the instance.
	ports: Vec<Arc<ReentrantMutex<Port>>>,
	/// The instance's architecture handle.
	handle: A::InstanceHandle,
}

impl<A: Arch> Instance<A> {
	/// Creates a new instance, allocating a new mapper.
	///
	/// Notably, this does **not** spawn any threads.
	pub fn new(
		module: &Arc<ReentrantMutex<Module<A>>>,
		ring: &Arc<ReentrantMutex<Ring<A>>>,
	) -> Result<Arc<ReentrantMutex<Self>>, MapError> {
		let id = crate::id::allocate();

		let mapper = AddressSpace::<A>::new_user_space(Kernel::<A>::get().mapper())
			.ok_or(MapError::OutOfMemory)?;

		let handle = A::InstanceHandle::new(mapper)?;

		// Apply the ring mapper overlay to the instance.
		AddressSpace::<A>::sysabi()
			.apply_user_space_shallow(handle.mapper(), ring.lock().mapper())?;

		// Apply the module's read-only mapper overlay to the instance.
		AddressSpace::<A>::user_rodata()
			.apply_user_space_shallow(handle.mapper(), module.lock().mapper())?;

		let r = Arc::new(ReentrantMutex::new(Self {
			id,
			module: module.clone(),
			ring: Arc::downgrade(ring),
			threads: Vec::new(),
			ports: Vec::new(),
			handle,
		}));

		ring.lock().instances_mut().push(r.clone());
		module.lock().instances.push(Arc::downgrade(&r));
		Kernel::<A>::get()
			.state()
			.instances
			.lock()
			.push(Arc::downgrade(&r));

		Ok(r)
	}

	/// Returns the instance ID.
	#[must_use]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// The handle to the module from which this instance was spawned.
	pub fn module(&self) -> Arc<ReentrantMutex<Module<A>>> {
		self.module.clone()
	}

	/// The weak handle to the ring on which this instance resides.
	pub fn ring(&self) -> Weak<ReentrantMutex<Ring<A>>> {
		self.ring.clone()
	}

	/// Gets a handle to the list of threads for this instance.
	pub fn threads(&self) -> &[Arc<ReentrantMutex<Thread<A>>>] {
		&self.threads
	}

	/// Gets a handle to the list of ports for this instance.
	pub fn ports(&self) -> &[Arc<ReentrantMutex<Port>>] {
		&self.ports
	}

	/// Returns the instance's address space handle.
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		self.handle.mapper()
	}
}
