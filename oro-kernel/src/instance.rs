//! Module instance types and functionality.

use oro_mem::mapper::{AddressSegment, AddressSpace as _, MapError};

use crate::{
	AddressSpace, Kernel, UserHandle,
	arch::{Arch, InstanceHandle},
	module::Module,
	ring::Ring,
	tab::Tab,
	table::{Table, TypeTable},
	thread::Thread,
	token::Token,
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
	/// The module from which this instance was spawned.
	///
	/// Strong reference to prevent the module from being
	/// deallocated while the instance is still alive, which would
	/// otherwise reclaim the executable memory pages and wreak havoc.
	module: Tab<Module<A>>,
	/// The ring on which this instance resides.
	ring: Tab<Ring<A>>,
	/// The thread list for the instance.
	pub(super) threads: Table<Tab<Thread<A>>>,
	/// The instance's architecture handle.
	handle: A::InstanceHandle,
	/// The instance's associated data.
	data: TypeTable,
	/// Memory tokens owned by this instance.
	///
	/// If a token is here, the instance is allowed to map it
	/// into its address space.
	tokens: Table<Tab<Token>>,
}

impl<A: Arch> Instance<A> {
	/// Creates a new instance, allocating a new mapper.
	///
	/// Notably, this does **not** spawn any threads.
	pub fn new(module: &Tab<Module<A>>, ring: &Tab<Ring<A>>) -> Result<Tab<Self>, MapError> {
		let mapper = AddressSpace::<A>::new_user_space(Kernel::<A>::get().mapper())
			.ok_or(MapError::OutOfMemory)?;

		let handle = A::InstanceHandle::new(mapper)?;

		// Apply the ring mapper overlay to the instance.
		ring.with(|ring| {
			AddressSpace::<A>::sysabi().apply_user_space_shallow(handle.mapper(), ring.mapper())
		})?;

		// Apply the module's read-only mapper overlay to the instance.
		module.with(|module| {
			AddressSpace::<A>::user_rodata()
				.apply_user_space_shallow(handle.mapper(), module.mapper())
		})?;

		let tab = crate::tab::get()
			.add(Self {
				module: module.clone(),
				ring: ring.clone(),
				threads: Table::new(),
				handle,
				data: TypeTable::new(),
				tokens: Table::new(),
			})
			.ok_or(MapError::OutOfMemory)?;

		ring.with_mut(|ring| ring.instances_mut().push(tab.clone()));

		Ok(tab)
	}

	/// The handle to the module from which this instance was spawned.
	pub fn module(&self) -> &Tab<Module<A>> {
		&self.module
	}

	/// The handle to the ring on which this instance resides.
	pub fn ring(&self) -> &Tab<Ring<A>> {
		&self.ring
	}

	/// Gets a handle to the list of threads for this instance.
	pub fn threads(&self) -> &Table<Tab<Thread<A>>> {
		&self.threads
	}

	/// Attempts to return a [`Token`] from the instance's token list.
	///
	/// Returns `None` if the token is not present.
	#[inline]
	#[must_use]
	pub fn token(&self, id: u64) -> Option<Tab<Token>> {
		self.tokens.get(id).cloned()
	}

	/// "Forgets" a [`Token`] from the instance's token list.
	///
	/// Returns the forgotten token, or `None` if the token is not present.
	#[inline]
	pub fn forget_token(&mut self, id: u64) -> Option<Tab<Token>> {
		self.tokens.remove(id)
	}

	/// Inserts a [`Token`] into the instance's token list.
	///
	/// Returns the ID of the token.
	#[inline]
	pub fn insert_token(&mut self, token: Tab<Token>) -> u64 {
		self.tokens.insert_tab(token)
	}

	/// Returns the instance's address space handle.
	#[inline]
	#[must_use]
	pub fn mapper(&self) -> &UserHandle<A> {
		self.handle.mapper()
	}

	/// Returns a reference to the instance's data.
	#[inline]
	pub fn data(&self) -> &TypeTable {
		&self.data
	}

	/// Returns a mutable reference to the instance's data.
	#[inline]
	pub fn data_mut(&mut self) -> &mut TypeTable {
		&mut self.data
	}
}
