//! Module instance types and functionality.

use oro_mem::{
	mapper::{AddressSegment, AddressSpace as _, MapError},
	phys::PhysAddr,
};

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
	/// The virtual memory mappings of virtual addresses to tokens.
	/// Values are `(token, page_id)` pairs.
	// TODO(qix-): This assumes a 4096 byte page size, and is also
	// TODO(qix-): not a very efficient data structure. It will be
	// TODO(qix-): replaced with a more efficient data structure
	// TODO(qix-): in the future.
	token_vmap: Table<(Tab<Token>, usize)>,
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

	/// Maps (sets the base of and reserves) a [`Token`] into the instance's address space.
	///
	/// **This does not immediately map in any memory.** It only marks the range
	/// in the _internal kernel_ address space as reserved for the token, to be
	/// committed later (typically via a page fault calling [`Instance::try_commit_token_at`]).
	pub fn try_map_token_at(
		&mut self,
		token: &Tab<Token>,
		virt: usize,
	) -> Result<(), TokenMapError> {
		if !self.tokens.contains(token.id()) {
			return Err(TokenMapError::BadToken);
		}

		token.with(|t| {
			match t {
				Token::Normal(t) => {
					debug_assert!(
						t.page_size() == 4096,
						"page size != 4096 is not implemented"
					);
					debug_assert!(
						t.page_size().is_power_of_two(),
						"page size is not a power of 2"
					);

					if (virt & (t.page_size() - 1)) != 0 {
						return Err(TokenMapError::VirtNotAligned);
					}

					let segment = AddressSpace::<A>::user_data();

					// Make sure that none of the tokens exist in the vmap.
					for page_idx in 0..t.page_count() {
						let page_base = virt + (page_idx * t.page_size());

						if !virt_resides_within::<A>(&segment, page_base)
							|| !virt_resides_within::<A>(&segment, page_base + t.page_size() - 1)
						{
							return Err(TokenMapError::VirtOutOfRange);
						}

						if self.token_vmap.contains(
							u64::try_from(page_base).map_err(|_| TokenMapError::VirtOutOfRange)?,
						) {
							return Err(TokenMapError::Conflict);
						}
					}

					// Everything's okay, map them into the vmap now.
					for page_idx in 0..t.page_count() {
						let page_base = virt + (page_idx * t.page_size());
						// NOTE(qix-): We can use `as` here since we already check the page base above.
						self.token_vmap
							.insert(page_base as u64, (token.clone(), page_idx));
					}

					Ok(())
				}
			}
		})
	}

	/// Commits a [`Token`] into the instance's address space at
	/// the specified virtual address. Returns a mapping error
	/// if the mapping could not be completed.
	///
	/// **The `maybe_unaligned_virt` parameter is not guaranteed to be aligned
	/// to any page boundary.** In most cases, it is coming directly from a
	/// userspace application (typically via a fault).
	///
	/// The `Token` must have been previously mapped via [`Instance::try_map_token_at`],
	/// or else this method will fail.
	pub fn try_commit_token_at(&self, maybe_unaligned_virt: usize) -> Result<(), TryCommitError> {
		// TODO(qix-): We always assume a 4096 page boundary. This will change in the future.
		let virt = maybe_unaligned_virt & !0xFFF;

		if let Some((token, page_idx)) = u64::try_from(virt)
			.ok()
			.and_then(|virt| self.token_vmap.get(virt))
		{
			token.with_mut(|t| {
				match t {
					Token::Normal(t) => {
						debug_assert!(*page_idx < t.page_count());
						debug_assert!(
							t.page_size() == 4096,
							"page size != 4096 is not implemented"
						);
						let page_base = virt + (*page_idx * t.page_size());
						let segment = AddressSpace::<A>::user_data();
						let phys = t
							.get_or_allocate(*page_idx)
							.ok_or(TryCommitError::MapError(MapError::OutOfMemory))?;
						segment
							.map(self.handle.mapper(), page_base, phys.address_u64())
							.map_err(TryCommitError::MapError)?;
						Ok(())
					}
				}
			})
		} else {
			Err(TryCommitError::BadVirt)
		}
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

/// An error returned by [`Instance::try_map_token_at`].
#[derive(Debug, Clone, Copy)]
pub enum TokenMapError {
	/// The virtual address is not aligned.
	VirtNotAligned,
	/// The virtual address is out of range for the
	/// thread's address space.
	VirtOutOfRange,
	/// The token was not found for the instance.
	BadToken,
	/// The mapping conflicts (overlaps) with another mapping.
	Conflict,
}

/// Checks if the given virtual address resides within the given address segment.
#[inline]
fn virt_resides_within<A: Arch>(
	segment: &<AddressSpace<A> as ::oro_mem::mapper::AddressSpace>::UserSegment,
	virt: usize,
) -> bool {
	// NOTE(qix-): Range is *inclusive*.
	let (first, last) = segment.range();
	virt >= first && virt <= last
}

/// An error returned by [`Instance::try_commit_token_at`].
#[derive(Debug, Clone, Copy)]
pub enum TryCommitError {
	/// The virtual address was not found in the virtual map.
	BadVirt,
	/// Mapping the token failed.
	MapError(MapError),
}
