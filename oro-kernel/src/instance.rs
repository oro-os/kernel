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
	token::{NormalToken, Token},
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
	/// DOES cover thread-local mappings. This is managed exclusively
	/// by [`Thread`]s; tokens here are shared between all threads in the instance.
	///
	/// If a token is here, the instance is allowed to map it
	/// into its address space.
	pub(super) tokens: Table<Tab<Token>>,
	/// The virtual memory mappings of virtual addresses to tokens.
	///
	/// Does NOT cover thread-local mappings. This is managed exclusively
	/// by [`Thread`]s; tokens here are shared between all threads in the instance.
	///
	/// Values are `(token, page_id)` pairs.
	// TODO(qix-): This assumes a 4096 byte page size, and is also
	// TODO(qix-): not a very efficient data structure. It will be
	// TODO(qix-): replaced with a more efficient data structure
	// TODO(qix-): in the future.
	pub(super) token_vmap: Table<(Tab<Token>, usize)>,
}

impl<A: Arch> Instance<A> {
	/// Creates a new instance, allocating a new mapper.
	///
	/// Notably, this does **not** spawn any threads.
	pub fn new(module: &Tab<Module<A>>, ring: &Tab<Ring<A>>) -> Result<Tab<Self>, MapError> {
		let mapper = AddressSpace::<A>::new_user_space(Kernel::<A>::get().mapper())
			.ok_or(MapError::OutOfMemory)?;

		let handle = A::InstanceHandle::new(mapper)?;

		// Apply the module's read-only mapper overlay to the instance.
		module.with(|module| {
			AddressSpace::<A>::user_rodata()
				.apply_user_space_shallow(handle.mapper(), module.mapper())
		})?;

		// Make the entire memory space shared.
		// TODO(qix-): This is a gross waste of memory, and will be addressed
		// TODO(qix-): in the future to be more fine-grained. I don't have a good
		// TODO(qix-): data structure written for random page fault fetches, so
		// TODO(qix-): instead we share all memory between all threads
		// TODO(qix-): in the instance, which requires around (255 * 4096) = 1MiB
		// TODO(qix-): of memory per instance. This isn't ideal, but it works for now.
		AddressSpace::<A>::user_data().provision_as_shared(handle.mapper())?;

		let tab = crate::tab::get()
			.add(Self {
				module: module.clone(),
				ring: ring.clone(),
				threads: Table::new(),
				handle,
				data: TypeTable::new(),
				tokens: Table::new(),
				token_vmap: Table::new(),
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

	/// Allocates a thread stack for the instance.
	///
	/// Upon success, returns the virtual address of the stack base.
	#[inline]
	pub(crate) fn allocate_stack(&mut self) -> Result<usize, MapError> {
		self.allocate_stack_with_size(65536)
	}

	/// Allocates a thread stack with the given size for the instance.
	///
	/// The size will be rounded up to the nearest page size, at the discretion
	/// of the kernel _or_ architecture implementation.
	///
	/// Note that `Err(MapError::VirtOutOfRange)` is returned if there is no more
	/// virtual address space available for the stack, and thus the thread cannot
	/// be spawned.
	#[cold]
	pub(crate) fn allocate_stack_with_size(&mut self, size: usize) -> Result<usize, MapError> {
		// Create the memory token.
		let page_count = ((size + 4095) & !4095) >> 12;
		let token = crate::tab::get()
			.add(Token::NormalThreadStack(NormalToken::new_4kib(page_count)))
			.ok_or(MapError::OutOfMemory)?;

		// Find an appropriate stack start address.
		let (thread_stack_low, thread_stack_high) = AddressSpace::<A>::user_thread_stack().range();
		let thread_stack_low = (thread_stack_low + 4095) & !4095;
		let thread_stack_high = thread_stack_high & !4095;

		let mut stack_base = thread_stack_high;

		'base_search: while stack_base > thread_stack_low {
			// Account for the high guard page.
			let stack_max = stack_base - 4096;
			// Allocate the stack + 1 for the lower guard page.
			let stack_min = stack_max - ((page_count + 1) << 12);

			// Try to see if all pages are available for token mapping.
			for addr in (stack_min..=stack_max).step_by(4096) {
				debug_assert!(addr & 4095 == 0);

				if self.token_vmap.contains(addr as u64) {
					// Stack cannot be allocated here; would conflict.
					// TODO(qix-): Get the mapping that conflicted and skip that many
					// TODO(qix-): pages. Right now we do the naive thing and search WAY
					// TODO(qix-): too many times, but I'm trying to implement this quickly
					// TODO(qix-): for now.
					stack_base -= 4096;
					continue 'base_search;
				}
			}

			// Insert it into the token map.
			debug_assert_ne!(
				stack_min + 4096,
				stack_max,
				"thread would allocate no stack pages (excluding guard pages)"
			);
			debug_assert_eq!(((stack_max) - (stack_min + 4096)) >> 12, page_count);

			for (page_idx, addr) in ((stack_min + 4096)..stack_max).step_by(4096).enumerate() {
				debug_assert!(addr & 4095 == 0);

				self.token_vmap
					.insert(addr as u64, (token.clone(), page_idx));
			}

			return Ok(stack_max);
		}

		Err(MapError::VirtOutOfRange)
	}
}
