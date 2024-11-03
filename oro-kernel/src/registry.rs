//! Implements types for the Oro registries (reference-counted
//! arena allocators).

// NOTE(qix-): This is an **INCREDIBLY UNSAFE** module. It is designed
// NOTE(qix-): to be as ergonomic and safe as possible from the outset,
// NOTE(qix-): especially for how it'll be used within Oro, but it employs
// NOTE(qix-): some normaally very questionable practices to achieve this
// NOTE(qix-): in order to stay performant in the hot path (optimizing
// NOTE(qix-): reads over writes, minimizing locking whilst doing so,
// NOTE(qix-): for example).
// NOTE(qix-):
// NOTE(qix-): It makes a ton of assumptions about its usage, and is NOT
// NOTE(qix-): suitable for use in any context other than the Oro kernel.
// NOTE(qix-):
// NOTE(qix-): DO NOT COPY THIS CODE INTO YOUR OWN PROJECTS IN ANY CAPACITY.
// NOTE(qix-): IT IS NOT SAFE. YOU HAVE BEEN WARNED.
// NOTE(qix-):
// NOTE(qix-): Similarly, if you are here to make edits to this code, please
// NOTE(qix-): be very careful and ensure that you are not introducing any
// NOTE(qix-): unsafety into the codebase. This is a very delicate module.
// NOTE(qix-): It will be HEAVILY scrutinized in code review. Be ready.

use core::{
	marker::PhantomData,
	mem::{ManuallyDrop, MaybeUninit, size_of},
	ops::Deref,
	sync::atomic::{AtomicUsize, Ordering},
};

use oro_macro::unlikely;
use oro_mem::{
	alloc::GlobalPfa,
	mapper::{AddressSegment, AddressSpace, MapError},
	pfa::Alloc,
};
use oro_sync::{Lock, TicketMutex};

use crate::{AddrSpace, Arch, SupervisorHandle, SupervisorSegment};

/// A registry for reference-counted arena allocation.
///
/// The registry is a reference-counted arena allocator that
/// allows for the allocation of items that are reference-counted
/// across the system. The registry is designed to be used in
/// a supervisor space, and is not intended for use in user space.
///
/// Registry allocations return [`Handle`]s, which can be cloned
/// and will free the slot when the final user drops it.
pub(crate) struct Registry<T: Send + Sized + 'static, A: Arch> {
	/// The base address of the registry.
	// TODO(qix-): Remove this field once const trait functions are stabilized,
	// TODO(qix-): replacing it with `segment.range().0 as *mut _` and saving
	// TODO(qix-): a few bytes.
	base: *mut MaybeUninit<ItemFrame<T>>,
	/// Bookkeeping counters used in the registry.
	bookkeeping: TicketMutex<RegistryBookkeeping>,
	/// The segment this registry is in.
	segment:     SupervisorSegment<A>,
	/// The mapper for the registry.
	mapper:      SupervisorHandle<A>,
	/// The architecture trait type
	_arch:       PhantomData<A>,
}

/// Registry-level bookkeeping fields, protected
/// behind a [`TicketMutex`].
struct RegistryBookkeeping {
	/// The last free ID in the registry.
	///
	/// If this is `usize::MAX`, then there are no free slots.
	last_free_id:     usize,
	/// The total count of items in the registry.
	total_count:      usize,
	/// Total page count of the registry.
	total_page_count: usize,
}

impl RegistryBookkeeping {
	/// Creates a new instance of the registry bookkeeping.
	fn new() -> Self {
		Self {
			last_free_id:     usize::MAX,
			total_count:      0,
			total_page_count: 0,
		}
	}
}

/// A frame in the registry.
///
/// Wraps an item `T` with metadata about the slot itself,
/// used for bookkeeping purposes.
struct ItemFrame<T: Send + Sized + 'static> {
	/// A union of the item or the next free index.
	maybe_item: MaybeItem<T>,
	/// Count of users of this item.
	/// In the event that this is zero, the item is free.
	/// In the event that this count reaches zero, the item gets dropped.
	user_count: AtomicUsize,
}

/// A union of either an occupied item slot, or the index of the
/// next free slot.
union MaybeItem<T: Send + Sized + 'static> {
	/// The item itself.
	item:      ManuallyDrop<TicketMutex<T>>,
	/// The next free index.
	next_free: usize,
}

impl<T: Send + Sized + 'static, A: Arch> Registry<T, A> {
	/// Creates a new, empty registry in the given
	/// segment.
	///
	/// Makes the registry available for use across all
	/// cores in the system.
	///
	/// The segment used for the registry must be valid,
	/// unique to all other registries, and previously
	/// unpopulated (or this function will error with
	/// [`MapError::Exists`]).
	///
	/// Typically, this function should be called once
	/// at boot time.
	pub fn new(segment: SupervisorSegment<A>) -> Result<Self, MapError> {
		// SAFETY(qix-): We can more or less guarantee that this registry
		// SAFETY(qix-): is being constructed in the supervisor space.
		// SAFETY(qix-): Further, we can't guarantee that the segment is
		// SAFETY(qix-): going to be accessed separately from other segments
		// SAFETY(qix-): quite yet, but we'll verify that we have exclusive
		// SAFETY(qix-): access to the segment directly after this call.
		let mapper = unsafe { AddrSpace::<A>::current_supervisor_space() };
		segment.provision_as_shared(&mapper)?;

		Ok(Self {
			base: segment.range().0 as *mut _,
			bookkeeping: TicketMutex::new(RegistryBookkeeping::new()),
			segment,
			mapper,
			_arch: PhantomData,
		})
	}

	/// Allocates and inserts an item `T` into the registry.
	///
	/// Returns an error if there was a problem allocating the item.
	///
	/// Takes a reference to the spinlock itself, since not all allocations require
	/// locking the PFA.
	pub fn insert(&'static self, item: impl Into<T>) -> Result<Handle<T>, MapError> {
		let item = item.into();

		let mut bk = self.bookkeeping.lock();

		let id = if bk.last_free_id == usize::MAX {
			let byte_offset = bk.total_count * size_of::<MaybeUninit<ItemFrame<T>>>();
			let byte_offset_end = byte_offset + size_of::<MaybeUninit<ItemFrame<T>>>();

			if unlikely!((self.segment.range().0 + byte_offset_end - 1) > self.segment.range().1) {
				return Err(MapError::VirtOutOfRange);
			}

			// TODO(qix-): If PFAs ever support more than 4K pages, this will need to be updated.
			let new_page_end = byte_offset_end & !4095;
			let new_page_count = new_page_end + 1;

			if new_page_count > bk.total_page_count {
				for page_id in bk.total_page_count..new_page_count {
					let page = GlobalPfa.allocate().ok_or(MapError::OutOfMemory)?;

					// TODO(qix-): If PFAs ever support more than 4K pages, this will need to be updated.
					let virt = self.segment.range().0 + page_id * 4096;
					if let Err(err) = self.segment.map(&self.mapper, virt, page) {
						// SAFETY(qix-): We just allocated this page and the mapper didn't use it.
						unsafe {
							GlobalPfa.free(page);
						}
						return Err(err);
					}

					// Increment on each loop such that if we fail, a future attempt won't try to
					// re-map the same virtual addresses.
					bk.total_page_count += 1;
				}
			}

			let id = bk.total_count;
			bk.total_count += 1;

			let slot = unsafe { &mut *self.base.add(id) };
			slot.write(ItemFrame {
				maybe_item: MaybeItem {
					item: ManuallyDrop::new(TicketMutex::new(item)),
				},
				user_count: AtomicUsize::new(1),
			});

			id
		} else {
			let id = bk.last_free_id;
			let slot = unsafe { (*self.base.add(id)).assume_init_mut() };
			bk.last_free_id = unsafe { slot.maybe_item.next_free };
			let last_user_count = slot.user_count.fetch_add(1, Ordering::Relaxed);
			debug_assert_eq!(last_user_count, 0);
			slot.maybe_item.item = ManuallyDrop::new(TicketMutex::new(item));

			id
		};

		Ok(Handle { id, registry: self })
	}

	/// Returns the item at the given ID, or `None` if the ID is invalid.
	///
	/// **This function incurs a registry lock.**
	/// You should use [`Handle`]s wherever possible, which do not
	/// incur registry locks.
	///
	/// # Safety
	/// **DO NOT PERFORM LOOKUPS BY ID FOR ANYTHING SECURITY-RELATED.**
	///
	/// IDs are RE-USABLE and may not refer to the same item if the item
	/// slot is dropped and re-allocated.
	///
	/// For that reason, this function is marked as unsafe.
	#[expect(dead_code)]
	pub unsafe fn get(&'static self, id: usize) -> Option<Handle<T>> {
		// We have to keep this lock open even during the lookup
		// since user counts are not locked at the record level
		// and there is no "fetch_and_increment_unless_zero" atomic
		// operation.
		//
		// NOTE(qix-): We could load and then do a compare-and-swap, but this function
		// NOTE(qix-): really should be seldom used, and I'm not interested in
		// NOTE(qix-): fleshing it out further at this time. PR welcome.
		let bk = self.bookkeeping.lock();

		if id >= bk.total_count {
			return None;
		}

		let slot = (*self.base.add(id)).assume_init_ref();

		// NOTE(qix-): Here's the part that could be changed
		// NOTE(qix-): to a compare-and-swap.
		if slot.user_count.load(Ordering::Relaxed) == 0 {
			None
		} else {
			slot.user_count.fetch_add(1, Ordering::Relaxed);
			Some(Handle { id, registry: self })
		}
	}
}

/// Handles item access and dropping in the registry.
trait RegistryAccess<T: Send + Sized + 'static> {
	/// Gets the item frame at the given ID.
	///
	/// # Safety
	/// Caller must ensure that the ID is valid.
	/// This function performs no bounds checks,
	/// and assumes if an ID is passed in, it is
	/// valid.
	unsafe fn get(&self, id: usize) -> &TicketMutex<T>;

	/// Increments the user count of the item at the given ID.
	///
	/// # Safety
	/// Caller must ensure that the ID is valid.
	/// This function performs no bounds checks,
	/// and assumes if an ID is passed in, it is
	/// valid.
	///
	/// The caller must ensure that [`Self::forget_item_at()`]
	/// is called when the item is no longer needed.
	unsafe fn lease_item_at(&self, id: usize);

	/// Forgets the item at the given ID.
	///
	/// If this is the last user of the item, the item
	/// will be dropped.
	///
	/// # Safety
	/// Caller must ensure that the ID is valid.
	/// This function performs no bounds checks,
	/// and assumes if an ID is passed in, it is
	/// valid.
	///
	/// Any references or handles to the item
	/// must be dropped before calling this function.
	unsafe fn forget_item_at(&self, id: usize);
}

impl<T: Send + Sized + 'static, A: Arch> RegistryAccess<T> for Registry<T, A> {
	unsafe fn get(&self, id: usize) -> &TicketMutex<T> {
		&(*self.base.add(id)).assume_init_ref().maybe_item.item
	}

	unsafe fn lease_item_at(&self, id: usize) {
		(*self.base.add(id))
			.assume_init_ref()
			.user_count
			.fetch_add(1, Ordering::Relaxed);
	}

	unsafe fn forget_item_at(&self, id: usize) {
		let slot = &mut *self.base.add(id);

		let last_user_count = slot
			.assume_init_ref()
			.user_count
			.fetch_sub(1, Ordering::Relaxed);

		// Should never be the case, as it means we somehow
		// missed dropping the item or otherwise mismanaged
		// the user count, which is a bug.
		debug_assert_ne!(last_user_count, 0);

		if last_user_count == 1 {
			let slot = slot.assume_init_mut();

			ManuallyDrop::drop(&mut slot.maybe_item.item);

			// SAFETY(qix-): DO NOT PUT THIS LOCK BEFORE THE ABOVE DROP.
			// SAFETY(qix-): YOU WILL DEADLOCK THE KERNEL.
			let mut bk = self.bookkeeping.lock();
			slot.maybe_item.next_free = bk.last_free_id;
			bk.last_free_id = id;
		}
	}
}

/// A lightweight handle to an item in a registry.
///
/// The handle is a reference-counted item in the registry,
/// and is a thin wrapper around an [`TicketMutex`]
/// to the item itself.
///
/// Handles can be safely `clone()`d. When the last handle
/// is dropped, the item is freed from the registry, where
/// the backing memory is reused for future allocations.
#[must_use]
pub struct Handle<T: Send + Sized + 'static> {
	/// The ID of the item in the registry.
	///
	/// This is the offset into the registry's base address.
	///
	/// **DO NOT USE THIS ID FOR ANYTHING SECURITY-SENSITIVE.**
	id:       usize,
	/// The registry the item is in.
	registry: &'static dyn RegistryAccess<T>,
}

// XXX(qix-): Temporary workaround to make things compile
// XXX(qix-): prior to switching to the heap allocators.
unsafe impl<T: Send + Sized + 'static> Send for Handle<T> {}
unsafe impl<T: Send + Sized + 'static> Sync for Handle<T> {}

impl<T: Send + Sized + 'static> Handle<T> {
	/// Returns the ID of the item in the registry.
	///
	/// **DO NOT USE THIS ID FOR ANYTHING SECURITY-SENSITIVE.**
	/// You should use `Handle`s wherever possible.
	///
	/// Note that this ID may go stale if the item is
	/// dropped and re-allocated. Future lookups
	/// using the given ID **_MAY_ NOT** refer to the
	/// same item.
	///
	/// **Do not rely on this ID for anything other
	/// than debugging or logging purposes.**
	#[must_use]
	pub fn id(&self) -> usize {
		self.id
	}
}

impl<T: Send + Sized + 'static> Deref for Handle<T> {
	type Target = TicketMutex<T>;

	fn deref(&self) -> &Self::Target {
		// SAFETY(qix-): We can assume that, given this handle
		// SAFETY(qix-): is even created (and cannot be created
		// SAFETY(qix-): externally), the ID is valid.
		unsafe { self.registry.get(self.id) }
	}
}

impl<T: Send + Sized + 'static> PartialEq for Handle<T> {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
			&& core::ptr::addr_eq(
				core::ptr::from_ref(self.registry),
				core::ptr::from_ref(other.registry),
			)
	}
}

impl<T: Send + Sized + 'static> core::fmt::Debug for Handle<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Handle")
			.field("id", &self.id)
			.finish_non_exhaustive()
	}
}

impl<T: Send + Sized + 'static> Clone for Handle<T> {
	fn clone(&self) -> Self {
		// SAFETY(qix-): We can assume that, given this handle
		// SAFETY(qix-): is even created (and cannot be created
		// SAFETY(qix-): externally), the ID is valid.
		unsafe {
			self.registry.lease_item_at(self.id);
		}

		Self {
			id:       self.id,
			registry: self.registry,
		}
	}
}

impl<T: Send + Sized + 'static> Drop for Handle<T> {
	fn drop(&mut self) {
		// SAFETY(qix-): We can assume that, given this handle
		// SAFETY(qix-): is even created (and cannot be created
		// SAFETY(qix-): externally), the ID is valid.
		unsafe {
			self.registry.forget_item_at(self.id);
		}
	}
}

/// Doubly linked collection adapter interface for a registry.
///
/// Collections are used via two registries:
/// - The item registry, storing type `T` items.
/// - The list registry, storing [`Item<T>`] items.
///
/// Then other structures that want to store independent
/// lists of type `T` would use this structure to manage
/// the list.
///
/// # Not a List
/// This type is not a list, set, or any other convention
/// datatype, despite looking like one.
///
/// It is meant to group together handles of type `T` in
/// a manner that works for arenas (specifically,
/// registries).
///
/// An `Item` is doubly linked, and there is no concept
/// of a "beginning" or "end" of the list - at least,
/// not in any managed way. Having a handle to an `Item<T>`
/// is not a handle to a list, but a handle to a single
/// item in a list, and traversal can go in either direction
/// depending on if `prev` or `next` is used.
///
/// There is no way to get the "first" item in the list
/// without traversal, and there is no way to get the "last"
/// in any other way, either.
///
/// This means that item collections can be merged, split,
/// spliced, re-ordered, etc. quite freely at the expense
/// of not providing common list-like operations (which
/// generally are _not_ needed in the Oro kernel).
///
/// # Loops
/// Items cannot directly be linked to themselves via the
/// accessor methods; all methods only take handles to the
/// underlying items. While this does not prevent duplicates
/// in the list, it does prevent loops.
///
/// For this reason, almost all methods that work on
/// `Handle<Item<T>>` will mutate the underlying registry
/// in some way.
pub struct Item<T: Send + Sized + 'static, A: Arch> {
	/// The list that owns this item.
	///
	/// `None` if the item does not belong to a list.
	list:   Option<Handle<List<T, A>>>,
	/// The previous item in the list, or `None` if there is no previous item.
	prev:   Option<Handle<Item<T, A>>>,
	/// The next item in the list, or `None` if there is no next item.
	next:   Option<Handle<Item<T, A>>>,
	/// The handle to the item in its respective registry.
	handle: Handle<T>,
}

impl<T: Send + Sized + 'static, A: Arch> Item<T, A> {
	/// Creates a new item with the given handle.
	///
	/// The item is not linked to any other items.
	#[must_use]
	fn new(handle: Handle<T>) -> Self {
		Self {
			list: None,
			prev: None,
			next: None,
			handle,
		}
	}

	/// Returns whether or not the item is in a list.
	#[must_use]
	pub fn in_list(&self) -> bool {
		self.list.is_some()
	}

	/// Returns the previous item in the list, or `None` if there is no previous item.
	#[must_use]
	pub fn prev(&self) -> Option<Handle<Item<T, A>>> {
		self.prev.clone()
	}

	/// Returns the next item in the list, or `None` if there is no next item.
	#[must_use]
	pub fn next(&self) -> Option<Handle<Item<T, A>>> {
		self.next.clone()
	}

	/// Returns the underlying handle to the item.
	pub fn handle(&self) -> &Handle<T> {
		&self.handle
	}
}

impl<T: Send + Sized + 'static, A: Arch> Deref for Item<T, A> {
	type Target = TicketMutex<T>;

	fn deref(&self) -> &Self::Target {
		&self.handle
	}
}

impl<T: Send + Sized + 'static, A: Arch> Handle<Item<T, A>> {
	/// Removes the item from the list.
	///
	/// Note that the underlying handle is still kept, including
	/// any handles to the list item directly (i.e. `Self`).
	pub fn delete(&self) {
		let mut lock = self.lock();
		if let Some(list) = lock.list.take() {
			let mut list_lock = list.lock();
			debug_assert!(list_lock.count > 0);
			list_lock.count -= 1;
			match (lock.prev.take(), lock.next.take()) {
				// Middle of the list.
				(Some(prev), Some(next)) => {
					let mut prev_lock = prev.lock();
					let mut next_lock = next.lock();

					debug_assert_eq!(prev_lock.next.as_ref(), Some(self));
					debug_assert_eq!(next_lock.prev.as_ref(), Some(self));

					prev_lock.next = Some(next.clone());
					next_lock.prev = Some(prev.clone());
				}
				// End of the list.
				(Some(prev), None) => {
					let mut prev_lock = prev.lock();

					debug_assert_eq!(prev_lock.next.as_ref(), Some(self));
					debug_assert_eq!(list_lock.last.as_ref(), Some(self));

					prev_lock.next = None;
					list_lock.last = Some(prev.clone());
				}
				// Beginning of the list.
				(None, Some(next)) => {
					let mut next_lock = next.lock();

					debug_assert_eq!(next_lock.prev.as_ref(), Some(self));
					debug_assert_eq!(list_lock.first.as_ref(), Some(self));

					next_lock.prev = None;
					list_lock.first = Some(next.clone());
				}
				// Only item in the list.
				(None, None) => {
					debug_assert_eq!(list_lock.first.as_ref(), Some(self));
					debug_assert_eq!(list_lock.last.as_ref(), Some(self));
					debug_assert_eq!(list_lock.count, 0);

					list_lock.first = None;
					list_lock.last = None;
				}
			}
		}
	}
}

/// A single list in a registry.
///
/// Holds [`Item`]s in a doubly linked list.
pub struct List<T: Send + Sized + 'static, A: Arch> {
	/// Holds a static reference to the underlying list item registry.
	item_registry: &'static Registry<Item<T, A>, A>,
	/// The first item in the list, or `None` if the list is empty.
	first:         Option<Handle<Item<T, A>>>,
	/// The last item in the list, or `None` if the list is empty.
	last:          Option<Handle<Item<T, A>>>,
	/// The count of items in the list.
	count:         usize,
}

// XXX(qix-): Temporary workaround to make things compile
// XXX(qix-): prior to switching to the heap allocators.
unsafe impl<T: Send + Sized + 'static, A: Arch> Send for List<T, A> {}
unsafe impl<T: Send + Sized + 'static, A: Arch> Sync for List<T, A> {}

impl<T: Send + Sized + 'static, A: Arch> Handle<List<T, A>> {
	/// Inserts an item to the end of the list.
	pub fn append(&self, item: Handle<T>) -> Result<Handle<Item<T, A>>, MapError> {
		let mut list_lock = self.lock();

		let item = list_lock.item_registry.insert(Item::new(item))?;

		{
			let last = list_lock.last.replace(item.clone());

			let mut item_lock = item.lock();
			item_lock.list = Some(self.clone());
			item_lock.prev = last;

			if list_lock.count == 0 {
				list_lock.first = Some(item.clone());
			}

			list_lock.count += 1;
		}

		Ok(item)
	}
}

impl<T: Send + Sized + 'static, A: Arch> List<T, A> {
	/// Creates a new, empty list
	fn new(item_registry: &'static Registry<Item<T, A>, A>) -> Self {
		Self {
			item_registry,
			first: None,
			last: None,
			count: 0,
		}
	}

	/// Returns the first item in the list, or `None` if the list is empty.
	#[must_use]
	pub fn first(&self) -> Option<Handle<Item<T, A>>> {
		self.first.clone()
	}
}

/// A wrapper around two registries to create lists and list items.
pub(crate) struct ListRegistry<T: Send + Sized + 'static, A: Arch> {
	/// The item registry.
	item_registry: Registry<Item<T, A>, A>,
	/// The list registry.
	// TODO(qix-): Change this to simply use `Self::List` once this is resolved:
	// TODO(qix-): https://github.com/rust-lang/rust/issues/108491
	list_registry: Registry<List<T, A>, A>,
}

impl<T: Send + Sized + 'static, A: Arch> ListRegistry<T, A> {
	/// Creates a new list registry.
	///
	/// The list registry is a pair of registries, one for
	/// items and one for lists. The item registry stores
	/// the items in the list, and the list registry stores
	/// the lists themselves.
	///
	/// The list registry is used to manage lists of items
	/// in a way that is safe and efficient for the Oro
	/// kernel.
	pub fn new(
		list_segment: SupervisorSegment<A>,
		item_segment: SupervisorSegment<A>,
	) -> Result<Self, MapError> {
		Ok(Self {
			item_registry: Registry::new(item_segment)?,
			list_registry: Registry::new(list_segment)?,
		})
	}

	/// Creates a new list in the list registry.
	///
	/// The list is empty when created.
	pub fn create_list(&'static self) -> Result<Handle<List<T, A>>, MapError> {
		self.list_registry.insert(List::new(&self.item_registry))
	}
}
