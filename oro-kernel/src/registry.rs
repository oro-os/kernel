//! Implements types for the Oro registries (reference-counted
//! arena allocators).

#![allow(dead_code)] // TODO(qix-)

// NOTE(qix-): This module is *VERY UNSAFE* and should be modified
// NOTE(qix-): with the utmost caution and care. It is one of the
// NOTE(qix-): cleanest ways I could find to encapsulate all of
// NOTE(qix-): the otherwise messy and error-prone shared allocation
// NOTE(qix-): logic in the kernel, and uses a few exotic tricks
// NOTE(qix-): to make it work (e.g. tightly coupling the registry
// NOTE(qix-): implementation to the kernel initialization routine
// NOTE(qix-): via `pub(super)`).
// NOTE(qix-):
// NOTE(qix-): Please do not try to "fix" this module unless you
// NOTE(qix-): fully understand the implications of the changes
// NOTE(qix-): you are making. The API has been decided on after
// NOTE(qix-): much consideration and is not likely to change
// NOTE(qix-): without a very good reason.
// NOTE(qix-):
// NOTE(qix-): Much of the unsafety in the module is due to the
// NOTE(qix-): lack of "generic statics" in Rust, which would allow
// NOTE(qix-): us to define a registry type generically across
// NOTE(qix-): all architectures. This is a limitation of the
// NOTE(qix-): language and is not likely to change in the near
// NOTE(qix-): future.

use core::{
	mem::MaybeUninit,
	sync::atomic::{AtomicU32, Ordering},
};
use oro_arch::Target;
use oro_common::{
	arch::Arch,
	mem::{
		mapper::{AddressSegment, AddressSpace, MapError},
		pfa::alloc::{PageFrameAllocate, PageFrameFree},
		translate::PhysicalAddressTranslator,
	},
	sync::spinlock::unfair_critical::UnfairCriticalSpinlock,
};

/// A registry, which is a reference-counted arena allocator.
///
/// Registries are the only way to allocate objects in the Oro
/// kernel's heap. They are inherently shared across cores,
/// such that references returned by the registry can be
/// safely sent between cores.
pub struct Registry<T, P, Alloc>
where
	T: RegistryTarget,
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree + 'static,
{
	/// The base pointer to the first slot in the arena.
	base:          *mut MaybeUninit<ArenaEntry<T>>,
	/// The number of slots in the arena.
	num_slots:     AtomicU32,
	/// The next index that is free,
	/// or `u32::MAX` if the registry is full.
	last_free:     AtomicU32,
	/// The page frame allocator to use for the registry.
	pfa:           &'static UnfairCriticalSpinlock<Alloc>,
	/// The address space segment within which pages will be allocated.
	segment:       <<Target as Arch>::AddressSpace as AddressSpace>::SupervisorSegment,
	/// The translator to use for physical addresses.
	translator:    P,
	/// Internal critical lock for the registry.
	registry_lock: UnfairCriticalSpinlock<()>,
}

impl<T, P, Alloc> Registry<T, P, Alloc>
where
	T: RegistryTarget,
	P: PhysicalAddressTranslator,
	Alloc: PageFrameAllocate + PageFrameFree + 'static,
{
	/// Creates a new registry.
	///
	/// # Safety
	/// This is meant to be called only once per registry,
	/// specifically by the [`RegistryTarget::initialize_registry()`]
	/// function.
	///
	/// For that reason, this function is `unsafe` and only accessible
	/// within the root module.
	pub(super) unsafe fn new(
		pfa: &'static UnfairCriticalSpinlock<Alloc>,
		translator: P,
		segment: <<Target as Arch>::AddressSpace as AddressSpace>::SupervisorSegment,
	) -> Self {
		let base = segment.range().0 as *mut ArenaEntry<T>;

		// NOTE(qix-): If this ever happens, I'd be shocked. As of writing,
		// NOTE(qix-): all of the address space segments are aligned to 4KiB
		// NOTE(qix-): boundaries (if not much more in the future), and the
		// NOTE(qix-): alignment of the registry types are never going to be
		// NOTE(qix-): anything close to 4KiB. If you're seeing this assert,
		// NOTE(qix-): something has gone horribly wrong. Please open an issue.
		assert_eq!(
			base.align_offset(core::mem::align_of::<ArenaEntry<T>>()),
			0,
			"Registry type alignment is not aligned with the segment base address"
		);

		Self {
			// SAFETY(qix-): This is not a valid pointer yet, but will be on first
			// SAFETY(qix-): allocation. Barring a bug in the API (or a misuse of
			// SAFETY(qix-): the API), we'll never be in a situation where this
			// SAFETY(qix-): gets dereferenced before a `Ref` is allocated (which
			// SAFETY(qix-): inserts, thus making the pointer valid).
			base: segment.range().0 as *mut MaybeUninit<ArenaEntry<T>>,
			num_slots: AtomicU32::new(0),
			last_free: AtomicU32::new(u32::MAX),
			pfa,
			segment,
			translator,
			registry_lock: UnfairCriticalSpinlock::new(()),
		}
	}

	/// Inserts an entry into the registry, potentially growing
	/// the registry if necessary.
	///
	/// Returns the index of the entry in the registry, or
	/// `None` if the registry is full.
	// SAFETY(qix-): There is no case save for a bug in the implementation
	// SAFETY(qix-): of address spaces where this function would panic.
	// SAFETY(qix-): We can thereby ignore the missing panics documentation
	// SAFETY(qix-): lint.
	//
	// SAFETY(qix-): THIS METHOD MAY NOT FREE. It is ONLY allowed to allocate.
	// SAFETY(qix-): FREEING PAGE FRAMES *WILL* CAUSE CACHE COHERENCY ISSUES
	// SAFETY(qix-): ACROSS CORES.
	//
	// SAFETY(qix-): This method is NOT re-entrant. Do not recurse.
	//
	// SAFETY(qix-): DO NOT PUBLICIZE (with `pub`).
	#[allow(clippy::missing_panics_doc)]
	fn insert(&self, entry: T) -> Option<u32> {
		// SAFETY(qix-): We specify this method is not re-entrant, so we can
		// SAFETY(qix-): safely lock the registry for the duration of the operation.
		//
		// SAFETY(qix-): This starts a critical section since we use a critical spinlock.
		let _registry_lock = unsafe { self.registry_lock.lock::<Target>() };

		let mut last_free = self.last_free.load(Ordering::Acquire);

		// Ensure we have a free slot. If not, grow the registry
		// and 'free' the new slots to register them in the list.
		if last_free == u32::MAX {
			// Otherwise, we can't cast safely.
			::oro_common::util::assertions::assert_fits_within::<u32, usize>();

			let num_slots_u32 = self.num_slots.load(Ordering::Acquire);
			// SAFETY(qix-): We can safely cast this to a usize since we've
			// SAFETY(qix-): statically checked that usize >= u32.
			let num_slots = num_slots_u32 as usize;

			let total_size_bytes = num_slots * core::mem::size_of::<ArenaEntry<T>>();
			// TODO(qix-): These assume a 4096-byte page size. They will have to be
			// TODO(qix-): adjusted if we ever support different page sizes.
			let total_size_bytes_page_aligned = (total_size_bytes + 4095) & !4095;
			let new_total_size_bytes_page_aligned = total_size_bytes_page_aligned + 4096;

			// NOTE(qix-): We have to calculate this because `T` may not perfectly fit
			// NOTE(qix-): into the page-aligned size of the registry at certain multiples.
			// NOTE(qix-):
			// NOTE(qix-): Division is banned in the kernel, so we do it this way.
			// SAFETY(qix-): We'll never exceed an isize here. We also do the modulo
			// SAFETY(qix-): to ensure that the two pointers are multiples of size_of<T>,
			// SAFETY(qix-): which is prescribed by `offset_from()`.
			#[allow(clippy::cast_sign_loss)]
			let new_total_slots = unsafe {
				self.base
					// Base is currently a pointer to ArenaEntry<T>, which means ::add() would add
					// `n * size_of::<ArenaEntry<T>>()` bytes to the pointer. We need to add
					// `n` bytes, so we have to cast it to a u8 pointer first...
					.cast::<u8>()
					// ... adding the bytes to it, making sure that it's a perfect multiple of
					// `size_of::<ArenaEntry<T>>`...
					.add(
						new_total_size_bytes_page_aligned
							- (new_total_size_bytes_page_aligned % core::mem::size_of::<MaybeUninit<ArenaEntry<T>>>()),
					)
					// ... cast it back to a pointer to ArenaEntry<T>...
					.cast::<MaybeUninit<ArenaEntry<T>>>()
					// ... and calculate the offset from the base pointer,
					// in units of `ArenaEntry<T>` (which we can do by adding directly
					// to the typed pointer instead of casting bytes) ...
					.offset_from(self.base.add(num_slots))
					// ... and then cast it back to a usize.
					// SAFETY(qix-): We know for a fact the offset is positive, so this is safe.
					as usize
			};

			let new_num_slots = new_total_slots - num_slots;

			// Would this overflow the u32 slot size? If so, we're out of memory.
			// In reality, we probably don't need to check this. However,
			// better safe than sorry.
			//
			// Note that we haven't performed any allocations yet, so there's
			// nothing special to do here (free, etc).
			// SAFETY(qix-): Barring some super huge page allocation (which would
			// SAFETY(qix-): never be the case on any modern architecture), this
			// SAFETY(qix-): will never overflow, so it's safe to cast.
			#[allow(clippy::cast_possible_truncation)]
			num_slots_u32.checked_add(new_num_slots as u32)?;

			// Gets the core-local address space handle.
			//
			// SAFETY(qix-): This is safe, if not a bit slow. We're only ever
			// SAFETY(qix-): modifying a segment we have exclusive access to,
			// SAFETY(qix-): assuming the architecture-specific address space
			// SAFETY(qix-): layouts are properly set up and implemented.
			// SAFETY(qix-):
			// SAFETY(qix-): While this *technically* violates the "exclusive
			// SAFETY(qix-): mutable reference" rule of Rust, we don't ever
			// SAFETY(qix-): actually modify page tables we don't own. This is
			// SAFETY(qix-): a bit of a hack, but it's the only way to make
			// SAFETY(qix-): this work without a lot of extra complexity.
			let mapper = unsafe {
				<<Target as Arch>::AddressSpace as AddressSpace>::current_supervisor_space(
					&self.translator,
				)
			};

			// Allocate a new physical page.
			// SAFETY(qix-): We're only obtaining the lock for this operation,
			// SAFETY(qix-): so this is safe.
			let phys = unsafe { self.pfa.lock::<Target>().allocate()? };

			// SAFETY(qix-): The pointer is not technically valid right at the add call,
			// SAFETY(qix-): but will be shortly after (when we map it in). The
			// SAFETY(qix-): size (in bytes) of the offset will also never approach `isize`
			// SAFETY(qix-): limits, either. Lastly, we prescribe that the registries
			// SAFETY(qix-): cannot border the address space beginnin/end, so we do not
			// SAFETY(qix-): run the risk of wrapping around (something `::add()` does not
			// SAFETY(qix-): check for). Thus, this is "safe" as long as we don't de-reference
			// SAFETY(qix-): the pointer before the page is mapped in.
			let virt =
				unsafe { self.base.cast::<u8>().add(total_size_bytes_page_aligned) as usize };

			// Map it in. We use `map_nofree` to make sure that we don't inadvertently
			// free any page frames that might have already been mapped into the shared
			// address space set up by the architectures for the registries (see
			// `Arch::make_segment_shared()`, which is called for each registry segment
			// upon initialization). Thus, we can guarantee that the TLB will be empty
			// for the page frame, for all cores, such that a page table walk will be
			// required to access the page frame on other cores, thus guaranteeing that
			// the allocation here will be seen by all cores, and the reference returned
			// will be available and safely passable to other cores after a memory barrier.
			//
			// TODO(qix-): This assumes a 4096-byte page size. This will have to be
			// TODO(qix-): adjusted if we ever support different page sizes.
			// SAFETY(qix-): We can safely take a lock out on the PFA here since mappers
			// SAFETY(qix-): are not allowed to panic.
			if let Err(err) = self.segment.map_nofree(
				&mapper,
				unsafe { &mut *self.pfa.lock::<Target>() },
				&self.translator,
				virt,
				phys,
			) {
				// Welp, something happened. Give the PFA back the page frame allocator.
				// SAFETY(qix-): We just allocated it, it's not being used, and thus it's
				// SAFETY(qix-): safe to free it. The lock is also won't panic while it's held.
				unsafe {
					self.pfa.lock::<Target>().free(phys);
				}

				match err {
					MapError::OutOfMemory | MapError::VirtOutOfAddressSpaceRange => {
						// Out of memory
						Target::strong_memory_barrier();
						return None;
					}
					unknown => {
						panic!("unexpected error mapping in new registry page: {unknown:?}");
					}
				}
			}

			// Now for each new slot, place-construct the entries.
			// We do it in reverse so that the new slots are at the
			// front of the free list (for neatness and to perhaps
			// reduce cache misses, though that's a bit of a stretch).
			for idx in (num_slots..new_num_slots).rev() {
				// SAFETY(qix-): We've just mapped in the page, so we can safely
				// SAFETY(qix-): write to it. We also know that the page is not
				// SAFETY(qix-): being used by any other core, so we can safely
				// SAFETY(qix-): write to it without any cache coherency issues.
				unsafe {
					let entry = &mut *self.base.add(idx);

					entry.write(ArenaEntry {
						value:     MaybeUninit::uninit(),
						ref_count: AtomicU32::new(0),
						next_free: AtomicU32::new(last_free),
					});
				}

				// Make sure we didn't mess up in our calculations or
				// bounds checks above. This should never happen.
				debug_assert!(u32::try_from(idx).is_ok());

				// SAFETY(qix-): We have sufficiently checked above that
				// SAFETY(qix-): the new indices are within the bounds of a u32.
				#[allow(clippy::cast_possible_truncation)]
				let idx = idx as u32;

				last_free = idx;
			}

			// Store everything back into the registry.
			// SAFETY(qix-): We can safely cast this to a u32 since we've
			// SAFETY(qix-): already ensured it hasn't overflowed.
			#[allow(clippy::cast_possible_truncation)]
			self.num_slots
				.store(new_total_slots as u32, Ordering::Release);
			// Technically not necessary but good to be defensive and explicit.
			// It will be immediately overwritten by the following code, but
			// we're keeping the spaceship flying.
			self.last_free.store(last_free, Ordering::Release);

			// Wait until all of our writes complete.
			Target::strong_memory_barrier();
		}

		// Make sure we didn't royally mess up; `last_free` should
		// definitely be a valid index.
		debug_assert_ne!(last_free, u32::MAX);

		// Great, take it off the free list.
		// SAFETY(qix-): We'll never approach an isize here, and we can
		// SAFETY(qix-): guarantee that the pointer is (now) valid after
		// SAFETY(qix-): the allocation. We can also safely cast to a
		// SAFETY(qix-): usize since we've statically checked that
		// SAFETY(qix-): usize >= u32. We can further guarantee that
		// SAFETY(qix-): the slot's been initialized by the above code,
		// SAFETY(qix-): so `assume_init_mut()` is safe.
		let slot = unsafe { (*self.base.add(last_free as usize)).assume_init_mut() };

		self.last_free
			.store(slot.next_free.load(Ordering::Acquire), Ordering::Release);
		slot.next_free.store(u32::MAX, Ordering::Release);

		let previous_refcount = slot.ref_count.fetch_add(1, Ordering::Release);
		// This should always be the case barring a bug in the implementation.
		debug_assert_eq!(previous_refcount, 0);

		slot.value.write(UnfairCriticalSpinlock::new(entry));

		// Wait until all of our writes complete.
		Target::strong_memory_barrier();

		Some(last_free)
	}
}

/// A single entry in an arena.
#[repr(C)]
struct ArenaEntry<T: Sized> {
	/// The object in the arena.
	// SAFETY(qix-): This MUST be the first field in the struct.
	value: MaybeUninit<UnfairCriticalSpinlock<T>>,
	/// The reference count of the object.
	ref_count: AtomicU32,
	/// The next index that is free, or `u32::MAX` if this
	/// was least-recently freed bucket.
	next_free: AtomicU32,
}

/// Implementations of this trait can be allocated in a registry.
///
/// # Safety
/// This trait is unsafe because it's not meant to be implemented willy-nilly,
/// but rather by types that are meant to be allocated in a registry, whereby
/// the registry itself is closely guarded, and where the initialization
/// is done in a safe manner.
///
/// For that reason, this trait is `unsafe` and only accessible within the
/// root module.
///
/// **DO NOT USE THIS TRAIT ANYWHERE ELSE IN THE KERNEL CODE.**
pub(super) unsafe trait RegistryTarget: Sized {
	/// The type of allocator to use.
	type Alloc: PageFrameAllocate + PageFrameFree + 'static;
	/// The type of physical address translator to use.
	type PhysicalAddressTranslator: PhysicalAddressTranslator + 'static;

	/// The raw pointer to the registry.
	// SAFETY(qix-): This seems like a bug in Clippy; we're not declaring
	// SAFETY(qix-): an inner mutable const here, but a *pointer* to one.
	// SAFETY(qix-): https://github.com/rust-lang/rust-clippy/issues/13233
	#[allow(clippy::declare_interior_mutable_const)]
	const REGISTRY_PTR: *const crate::registry::Registry<
		Self,
		Self::PhysicalAddressTranslator,
		Self::Alloc,
	>;

	/// Initializes the registry
	///
	/// # Safety
	/// This function must be called EXACTLY ONCE for each registry.
	///
	/// The segment passed to this function **MUST NOT OVERLAP** with
	/// any other segment (including other registries) in the address space.
	/// Doing so WILL incur undefined behavior under Rust's safety rules
	/// (and already toes the line as it is).
	unsafe fn initialize_registry(
		segment: <<Target as Arch>::AddressSpace as AddressSpace>::SupervisorSegment,
		translator: Self::PhysicalAddressTranslator,
	);
}

/// A reference to an object in a registry.
#[derive(Clone, Copy)]
pub struct Ref<T: RegistryTarget + Sized + 'static> {
	/// The index of the object in the registry.
	index:    u32,
	/// The pointer to the registry.
	registry: *const Registry<
		T,
		<T as RegistryTarget>::PhysicalAddressTranslator,
		<T as RegistryTarget>::Alloc,
	>,
}

impl<T: RegistryTarget + Sized + 'static> Ref<T> {
	/// Allocates a new object in the registry,
	/// taking ownership of the object and returning
	/// a [`Ref`] to it.
	///
	/// Returns `None` if the system is out of memory
	/// or if the registry would grow beyond its
	/// address space segment.
	// SAFETY(qix-): There is no case save for a bug in the implementation
	// SAFETY(qix-): of address spaces where this function would panic.
	// SAFETY(qix-): We can thereby ignore the missing panics documentation
	// SAFETY(qix-): lint.
	#[allow(clippy::missing_panics_doc)]
	pub fn from(v: T) -> Option<Ref<T>> {
		// NOTE(qix-): This is a confirmed bug in Clippy; we're not
		// NOTE(qix-): referencing an interior mutable const, but a
		// NOTE(qix-): pointer to one. A fix is in the works.
		// NOTE(qix-): https://github.com/rust-lang/rust-clippy/issues/13233
		#[allow(clippy::borrow_interior_mutable_const)]
		let registry = unsafe { &*T::REGISTRY_PTR };

		Some(Ref {
			index:    registry.insert(v)?,
			registry: <T as RegistryTarget>::REGISTRY_PTR,
		})
	}
}

// SAFETY(qix-): We ensure that `Ref` satisfies both of these
// SAFETY(qix-): traits by only ever taking immutable references
// SAFETY(qix-): to the registry, and by ensuring that the registry
// SAFETY(qix-): is only ever mutated in a safe manner.
unsafe impl<T: RegistryTarget + Sized + 'static> Send for Ref<T> {}
unsafe impl<T: RegistryTarget + Sized + 'static> Sync for Ref<T> {}

impl<T: RegistryTarget + Sized + 'static> PartialEq for Ref<T> {
	fn eq(&self, other: &Self) -> bool {
		// SAFETY(qix-): It is safe to ignore the 'registry' field
		// SAFETY(qix-): because there is only ever one registry
		// SAFETY(qix-): for each type, and the index is unique
		// SAFETY(qix-): for each object in the registry.
		self.index == other.index
	}
}

impl<T: RegistryTarget + Sized + 'static> Eq for Ref<T> {}
