//! Oro kernel object registry implementation.
#![expect(unused_imports)]

use core::{
	mem::MaybeUninit,
	sync::atomic::{
		AtomicBool,
		Ordering::{Acquire, Release},
	},
};

use oro_mem::alloc::{
	collections::btree_map::BTreeMap,
	sync::{Arc, Weak},
};
use oro_sync::{Lock, ReentrantMutex};
use oro_sysabi::{
	key,
	syscall::{Error, Opcode, Result},
};

use crate::{
	arch::Arch,
	interface::{Interface, SystemCallAction, SystemCallRequest, SystemCallResponse},
	table::Table,
	thread::Thread,
};

/// A system-wide "master" registry.
///
/// Holds all interface handles in the system,
/// each identified by a unique ID that monotonically
/// increases upon insertion via [`crate::id::allocate()`].
///
/// The lower level entity types (rings, instances, etc.) hold
/// a view into this registry, caching interfaces as needed
/// so as to reduce pressure on the registry's locks.
pub struct RootRegistry {
	/// A map of all registered interfaces.
	interface_map: Table<Arc<dyn Interface>>,
}

impl RootRegistry {
	/// Creates a new, fully empty registry.
	#[must_use]
	pub fn new_empty() -> Self {
		Self {
			interface_map: Table::new(),
		}
	}
}

/// Implements access to a [`RootRegistry`] or a [`RegistryView`]
/// (or some wrapper thereof).
pub trait Registry: Send {
	/// Inserts an interface into the registry and returns its globally unique ID.
	///
	/// The interface is guaranteed to be unique in the registry, and is registered
	/// globally.
	fn register_interface(&mut self, interface: Arc<dyn Interface>) -> u64;

	/// Looks up an interface by its globally unique ID. If this is a view,
	/// it may cache the interface for future lookups.
	fn lookup(&mut self, interface_id: u64) -> Option<Arc<dyn Interface>>;
}

impl Registry for RootRegistry {
	#[inline]
	fn register_interface(&mut self, interface: Arc<dyn Interface>) -> u64 {
		self.interface_map.insert_unique(interface)
	}

	#[inline]
	fn lookup(&mut self, interface_id: u64) -> Option<Arc<dyn Interface>> {
		self.interface_map.get(interface_id).cloned()
	}
}

/// A scoped / cached view into a parent [`Registry`].
///
/// Accesses to the registry through this type are cached,
/// reducing contention on the parent registry's locks.
pub struct RegistryView<P: Registry> {
	/// The parent registry from which to fetch interfaces.
	parent: Arc<ReentrantMutex<P>>,
	/// A cache of interfaces.
	// TODO(qix-): Use an LFU?
	cache: Table<Weak<dyn Interface>>,
}

impl<P: Registry> RegistryView<P> {
	/// Creates a new registry view into the given parent registry.
	pub fn new(parent: Arc<ReentrantMutex<P>>) -> Self {
		Self {
			parent,
			cache: Table::new(),
		}
	}
}

impl<P: Registry> Registry for RegistryView<P> {
	fn register_interface(&mut self, interface: Arc<dyn Interface>) -> u64 {
		let weak = Arc::downgrade(&interface);
		let id = self.parent.lock().register_interface(interface);
		// SAFETY: We can assume that the interface has not been inserted before, since
		// SAFETY: `register_interface` guarantees that the interface is unique.
		unsafe {
			self.cache.insert_unique_unchecked(id, weak);
		}
		id
	}

	fn lookup(&mut self, interface_id: u64) -> Option<Arc<dyn Interface>> {
		self.cache
			.get(interface_id)
			.and_then(Weak::upgrade)
			.or_else(|| {
				self.parent
					.lock()
					.lookup(interface_id)
					.inspect(|interface| {
						let weak = Arc::downgrade(interface);
						// SAFETY: We know it doesn't exist in this view because we just checked
						// SAFETY: and we currently have an exclusive (mutable) reference to `self`.
						unsafe {
							self.cache.insert_unique_unchecked(interface_id, weak);
						}
					})
			})
	}
}
