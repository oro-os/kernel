//! Global table.
//!
//! See the [`GlobalTable`] type for more information.
#![expect(private_bounds, clippy::redundant_else, clippy::inline_always)]

use core::{
	marker::PhantomData,
	mem::MaybeUninit,
	sync::atomic::{
		AtomicPtr, AtomicU64,
		Ordering::{self, Acquire, Relaxed, Release},
	},
};

use oro_macro::assert;
use oro_mem::{
	alloc::boxed::Box,
	pfa::Alloc,
	phys::{Phys, PhysAddr},
};

use crate::arch::Arch;

/// The maximum version before a slot is tombstoned.
///
/// # Debugging
/// To debug tombstone handling, set the feature `debug-tombs`.
/// This caps the version at 255 instead of the much higher default value.
// NOTE(qix-): These values must be a power of two minus one, as they are
// NOTE(qix-): also used as masks.
const MAX_VERSION_BEFORE_TOMBSTONE: u64 = {
	#[cfg(all(debug_assertions, feature = "debug-tombs"))]
	{
		255
	}
	#[cfg(not(all(debug_assertions, feature = "debug-tombs")))]
	{
		(1 << 29) - 1
	}
};

/// Returns a reference to the global table.
///
/// This table is guaranteed to exist for the lifetime of the kernel.
///
/// # Safety
/// For ergonomic reasons, this function is **not** marked as `unsafe`.
///
/// However, if the global allocator has not been initialized, any
/// attempt to insert into the global table will result in an OOM
/// error response.
#[inline]
#[must_use]
pub fn get() -> &'static GlobalTable {
	#[doc(hidden)]
	static GLOBAL_TABLE: GlobalTable = GlobalTable {
		counter:   AtomicU64::new(0),
		last_free: AtomicU64::new(!0),
		l0:        EncodedAtomicPtr::new(core::ptr::null_mut()),
	};

	&GLOBAL_TABLE
}

/// A (mostly) lock free handle table that can be used to store any kind of
/// data, uniquely identified by a `u64` key, called a "tab" (an internal
/// name; this is not a term used publicly).
///
/// This implementation is **lock free** except for page allocations, meaning
/// there is absolutely no contention between threads when inserting,
/// removing, or looking up items in the table except for in relatively "rare"
/// cases where new page(s) need to be allocated.
///
/// Despite it being lock-free, it is also **thread safe**. It is used
/// primarily as the backing lookup registry for active handles.
///
/// # Oro ID Design Details
/// - The high bit is always set. This is to prevent "static" IDs used by the
///   kernel from being confused with dynamic IDs.
/// - There can only be `2^(9*3+7)=17,179,869,184` tabs 'open' at a time.
///   This includes all rings, module instances, threads, ports, interfaces, etc.
/// - The first tab allocates 4 pages of memory.
///   - Every 128th tab allocates a new page.
///   - Every `512*128`th tab allocates a second, additional page.
///   - Every `512^2*128`th tab allocates a third, additional page.
///   - Every `512^3*128`th tab allocates a fourth, additional page.
/// - `512^3*128 == 2^(9*3+7)` is a fun fact.
/// - Each tab can have `64-(9*3+7)-1=29` bits worth of `2^29=536870912` versions.
///   A version is a number that is incremented every time a tab is removed and re-added,
///   used to prevent ABA problems. If the version overflows, the `zombie-tombs` feature
///   dictates what happens:
///   - If the feature is enabled, the tab is freed and the version is reset to 0.
///     **This has the potential to cause ABA problems in the long term.**
///   - If the feature is disabled, the version slot is permanently killed.
/// - Once all tabs in a page are used, the page is freed back to the system.
///
/// # Memory Leakage
/// This table has the very, very small chance to leak pages in the following
/// case:
///
/// 1. An entity is added to the table.
/// 2. During addition, two or more pages must be allocated.
/// 3. One or more of those pages fail to allocate.
/// 4. This happens repeatedly (e.g. > `128*512=65536` times) without
///    a single success in between.
///
/// There is a potential mitigation for this, but it is not implemented
/// as this is such an edge case that it's not worth the extra complexity at
/// this time. If this becomes a problem, it can be revisited - please file
/// an issue.
///
/// > **Note:** Oro is "safe and secure by default", especially in the kernel.
/// > This means that, while most users will not experience any issues with the
/// > `zombie-tombs` feature enabled (whereby versions wrap around, thus allowing
/// > stale IDs to be re-used), it is disabled by default. This is to prevent
/// > potential security issues from arising _by default_ unless opted-in.
pub struct GlobalTable {
	/// The global counter.
	counter:   AtomicU64,
	/// The last free slot, or `!0` if there are no free slots.
	last_free: AtomicU64,
	/// Pointers to the next level.
	l0:        EncodedAtomicPtr<SubTable<SubTable<SubTable<SlotList>>>>,
}

impl GlobalTable {
	/// Inserts an item into the table, returning its globally
	/// unique tab.
	///
	/// Returns `None` if the system is out of memory (or, in the
	/// extremely impossible case, if the table is completely full).
	// NOTE(qix-): this function doesn't really panic under any normal circumstances.
	#[expect(clippy::missing_panics_doc)]
	pub fn add<T: Tabbed>(&self, item: T) -> Option<Tab<T>> {
		let (slot, id) = 'get_slot: loop {
			let last_free = self.last_free.load(Relaxed);

			if last_free == !0 {
				let counter = self.counter.fetch_add(1, Relaxed);
				// NOTE(qix-): This should never happen in reality.
				debug_assert!(counter < (1 << 34), "out of tabs");
				let counter = (counter << 29) | (1 << 63);
				break 'get_slot (self.get_or_alloc_slot(counter as usize)?, counter);
			} else {
				// SAFETY: Barring a bug in this implementation, these loads are safe,
				// SAFETY: as free slots are guaranteed to have their backing pages
				// SAFETY: allocated.
				let slot = self.try_get_slot(last_free as usize).unwrap();

				// Try to read its last free slot.
				// SAFETY: We are immediately following this with a check and set.
				// SAFETY: If the check and set fails, the returned value is NOT
				// SAFETY: a slot index but instead some exposed pointer, and we
				// SAFETY: shall do nothing with it.
				let next_free = unsafe { slot.next_free() };

				if self
					.last_free
					.compare_exchange(last_free, next_free, Acquire, Relaxed)
					.is_err()
				{
					// Try again.
					continue;
				}

				// NOTE(qix-): We can only test this if we've ensured we've locked that free slot.
				// NOTE(qix-): It's just a sanity check to make sure, in some off chance, a race
				// NOTE(qix-): condition hasn't occurred.
				debug_assert_eq!(slot.ty(), TabType::Free);

				break 'get_slot (slot, last_free);
			}
		};

		// SAFETY: We just allocated this slot.
		let new_version = unsafe { slot.claim_unchecked(Box::into_raw(Box::new(item))) };

		// Replace the version in the ID.
		let id = (id & !((1 << 29) - 1)) | new_version;

		// SAFETY: We're passing `MUST_BE_FRESH=true`, so the tab constructor has
		// SAFETY: no additional preconditions here.
		Some(unsafe { Tab::new::<true>(id, slot) })
	}

	/// Frees a slot.
	///
	/// # Safety
	/// Must ONLY be called if the slot is ACTUALLY free.
	///
	/// The passed `slot` MUST correspond to the `id` passed.
	unsafe fn free(&self, id: u64, slot: &Slot) {
		#[cfg(debug_assertions)]
		{
			// Make sure the ID corresponds to the actual slot.
			let id_slot = self
				.try_get_slot(id as usize)
				.expect("precondition failed: `id` does not correspond to a live slot");
			debug_assert!(
				core::ptr::from_ref(id_slot) == slot,
				"precondition failed: `id` does not correspond to the passed slot"
			);
		}

		// Mark the slot as free. If this returns `true` the slot is
		// now a tomb.
		#[cfg(not(feature = "zombie-tombs"))]
		if slot.free_and_check_tomb() {
			// Go down the rabbit hole and see if any of the subtables are now all tombs.

			// TODO(qix-): Implement this. For now, we leak.
			oro_debug::dbg!("todo: tomb cleanup");

			// Do not mark as free.
			return;
		}

		#[cfg(feature = "zombie-tombs")]
		{
			// If the slot is a tomb, free it.
			if slot.free_and_check_tomb() {
				unreachable!("zombie-tombs is enabled but `free_and_check_tomb` returned true");
			}
		}

		// Mark as free.
		loop {
			let last_free = self.last_free.load(Relaxed);

			slot.set_next_free(last_free as usize);

			if self
				.last_free
				.compare_exchange(last_free, id, Release, Relaxed)
				.is_ok()
			{
				break;
			}
		}
	}

	/// Looks up a tab by its ID, returning an [`AnyTab`] if it exists.
	pub fn lookup_any(&self, id: u64) -> Option<AnyTab> {
		let slot = self.try_get_slot(id as usize)?;
		AnyTab::try_new(id, slot)
	}

	/// Looks up a tab by its ID, returning a [`Tab<T>`] if it exists.
	///
	/// Returns `None` if the types do not match.
	#[inline(always)]
	pub fn lookup<T: Tabbed>(&self, id: u64) -> Option<Tab<T>> {
		self.lookup_any(id)?.try_into()
	}

	/// Tries to get a slot, returning `None` if the slot is not allocated.
	fn try_get_slot(&self, counter: usize) -> Option<&Slot> {
		debug_assert_ne!(counter, 0);
		debug_assert_ne!(counter, !0);

		let Encoded::Live(l0) = self.l0.load(Relaxed) else {
			return None;
		};
		let Encoded::Live(l1) = l0.table[(counter >> 54) & 511].load(Relaxed) else {
			return None;
		};
		let Encoded::Live(l2) = l1.table[(counter >> 45) & 511].load(Relaxed) else {
			return None;
		};
		let Encoded::Live(sl) = l2.table[(counter >> 36) & 511].load(Relaxed) else {
			return None;
		};

		Some(&sl.slots[(counter >> 29) & 127])
	}

	/// Attempts to get the slot by its ID, allocating it (and any intermediaries)
	/// if it doesn't exist.
	fn get_or_alloc_slot(&self, counter: usize) -> Option<&Slot> {
		debug_assert_ne!(counter, 0);
		debug_assert_ne!(counter, !0);

		let Encoded::Live(l0) = self.l0.get_or_alloc_default()? else {
			unreachable!();
		};
		let Encoded::Live(l1) = l0.table[(counter >> 54) & 511].get_or_alloc_default()? else {
			unreachable!();
		};
		let Encoded::Live(l2) = l1.table[(counter >> 45) & 511].get_or_alloc_default()? else {
			unreachable!();
		};
		let Encoded::Live(sl) = l2.table[(counter >> 36) & 511].get_or_alloc_default()? else {
			unreachable!();
		};

		Some(&sl.slots[(counter >> 29) & 127])
	}
}

/// An "encoded" [`AtomicPtr`] wrapper that can be used to signal
/// nulls or tombstones.
#[derive(Default)]
#[repr(transparent)]
struct EncodedAtomicPtr<T: Default + 'static>(AtomicPtr<T>);

/// The state of an [`EncodedAtomicPtr`].
#[derive(Clone, Copy)]
enum Encoded<T: 'static> {
	/// The pointer is null.
	Null,
	/// The pointer is a tombstone.
	Tomb,
	/// The pointer is a valid pointer.
	Live(&'static T),
}

impl<T: Default + 'static> EncodedAtomicPtr<T> {
	/// Creates a new [`EncodedAtomicPtr`].
	#[inline(always)]
	const fn new(ptr: *mut T) -> Self {
		Self(AtomicPtr::new(ptr))
	}

	/// Gets the pointer, returning an [`Encoded`] value.
	///
	/// Always returns either [`Encoded::Tomb`] or [`Encoded::Live`].
	/// Never returns [`Encoded::Null`].
	fn get_or_alloc_default(&self) -> Option<Encoded<T>> {
		assert::size_of::<T, 4096>();

		let mut ptr = self.0.load(Relaxed);
		if ptr == (!0) as *mut T {
			return Some(Encoded::Tomb);
		} else if ptr.is_null() {
			let p_raw = ::oro_mem::global_alloc::GlobalPfa.allocate()?;
			// SAFETY: We just allocated this memory, so it's safe to use.
			let p = unsafe { Phys::from_address_unchecked(p_raw) };
			assert::aligns_to::<T, 4096>();
			// SAFETY: We're statically checking this directly above.
			let new_ptr: *mut T = unsafe { p.as_mut_ptr_unchecked() };
			// SAFETY: We just allocated this memory, so it's safe to write to it.
			unsafe {
				new_ptr.write(T::default());
			}
			if let Err(new_ptr) = self.0.compare_exchange(ptr, new_ptr, Relaxed, Relaxed) {
				// SAFETY: We just allocated this memory, so it's safe to free ourselves.
				unsafe {
					::oro_mem::global_alloc::GlobalPfa.free(p_raw);
				}
				ptr = new_ptr;
			} else {
				ptr = new_ptr;
			}
		}

		debug_assert!(!ptr.is_null());
		debug_assert!(ptr.is_aligned());

		// SAFETY: We control the allocation of the pointer, so this is safe.
		Some(Encoded::Live(unsafe { &*ptr }))
	}

	/// Loads the value from the underlying atomic,
	/// decoding any sentinel values as an [`Encoded`]
	/// value.
	#[inline]
	fn load(&self, ordering: Ordering) -> Encoded<T> {
		let ptr = self.0.load(ordering);
		if ptr.is_null() {
			Encoded::Null
		} else if ptr == (!0) as *mut T {
			Encoded::Tomb
		} else {
			// SAFETY: We control the allocation of the pointer, so this is safe.
			Encoded::Live(unsafe { &*ptr })
		}
	}
}

/// A [`Tab`]-able type, able to be stored in the [`GlobalTable`].
trait Tabbed {
	/// The type of handle this is.
	const TY: TabType;
}

/// A subtable, holding 512 entries to `AtomicPtr<T>`.
struct SubTable<T: Default + 'static> {
	/// The table.
	table: [EncodedAtomicPtr<T>; 512],
}

impl<T: Default + 'static> Default for SubTable<T> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Default + 'static> SubTable<T> {
	/// Creates a new subtable.
	fn new() -> Self {
		assert::size_of::<Self, 4096>();
		debug_assert!(get().last_free.load(Relaxed) == !0);

		// TODO(qix-): If this becomes a bottleneck, let's throw in a feature
		// TODO(qix-): called `null-is-zero` and zero it instead if enabled.
		let mut table: [MaybeUninit<EncodedAtomicPtr<T>>; 512] = MaybeUninit::uninit_array();
		for ptr in &mut table {
			ptr.write(EncodedAtomicPtr::new(core::ptr::null_mut()));
		}

		Self {
			table: unsafe { MaybeUninit::array_assume_init(table) },
		}
	}
}

/// An "any" [`Tab`], which only allows to read the type and to allow attempts
/// to convert it to its underlying tab type.
pub struct AnyTab {
	/// The tab's ID.
	id:  u64,
	/// The raw pointer to the slot.
	ptr: *const Slot,
}

// SAFETY: We can guarantee that this type is Send + Sync.
unsafe impl Send for AnyTab {}
// SAFETY: We can guarantee that this type is Send + Sync.
unsafe impl Sync for AnyTab {}

impl AnyTab {
	/// Creates a new [`AnyTab`].
	///
	/// Returns `None` if the slot is no longer valid, or if it is [`TabType::Free`].
	#[inline(always)]
	fn try_new(id: u64, ptr: *const Slot) -> Option<Self> {
		// SAFETY: We can guarantee that the slot is valid.
		let slot = unsafe { &*ptr };

		loop {
			let users = slot.users.load(Relaxed);
			if users == 0 {
				// The slot is no longer valid (we've successfully avoided a race condition!)
				return None;
			} else {
				// Try to increment the users count.
				// We do this since there's a small race condition
				// in the `Drop` implementation whereby the users count
				// is checked and the free occurs.
				//
				// 1. (THR A) Drop checks / decs the users count. It sees 0.
				// 2. (THR B) This function fetch-adds the users count. It thinks it has a lock.
				// 3. (THR A) Drop frees the slot.
				//
				// Further, even if the caller checks `ty()` after "locking"
				// this slot, the slot's `ver_ty` field may not have updated
				// to `Free` yet, causing even more subtle bugs.
				//
				// So instead, we just try to increment the users count;
				// with an `AnyTab`, it's only used in cases where we are
				// looking up a tab, so we know it should be non-zero.
				if slot
					.users
					.compare_exchange(users, users + 1, Relaxed, Relaxed)
					.is_ok()
				{
					break;
				}
			}
		}

		let this = Self { id, ptr };

		// We are now a valid user. Check the type.
		if this.ty() == TabType::Free {
			// The slot is free.
			return None;
		}

		Some(this)
	}

	/// The tab's ID.
	#[must_use]
	#[inline(always)]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// The tab's type.
	#[must_use]
	#[inline(always)]
	pub fn ty(&self) -> TabType {
		// SAFETY: We can guarantee that the slot is valid.
		unsafe { &*self.ptr }.ty()
	}

	/// Attempts to convert the [`AnyTab`] to a [`Tab<T>`].
	///
	/// Returns `None` if the types do not match.
	#[inline]
	#[must_use]
	pub fn try_into<T: Tabbed>(self) -> Option<Tab<T>> {
		if self.ty() != T::TY {
			return None;
		}

		// SAFETY: We are calling from a valid `AnyTab`,
		// SAFETY: so users count is guaranteed to be non-zero.
		Some(unsafe { Tab::new::<false>(self.id, self.ptr) })
	}
}

impl Clone for AnyTab {
	#[inline(always)]
	fn clone(&self) -> Self {
		// SAFETY: We can guarantee that the slot is valid.
		unsafe { &*self.ptr }.users.fetch_add(1, Relaxed);
		Self {
			id:  self.id,
			ptr: self.ptr,
		}
	}
}

impl Drop for AnyTab {
	fn drop(&mut self) {
		// SAFETY: We can guarantee that the slot is valid.
		let slot = unsafe { &*self.ptr };
		if slot.users.fetch_sub(1, Relaxed) == 1 {
			// SAFETY: We have the only owning reference to this slot,
			// SAFETY: and we control its construction; it's always going to
			// SAFETY: be the slot pointed to by the tab's ID.
			unsafe {
				get().free(self.id, slot);
			}
			// SAFETY(qix-): THE SLOT IS NO LONGER OURS TO USE.
			// SAFETY(qix-): FURTHER ACCESS TO THE SLOT IS UNDEFINED BEHAVIOR.
		}
	}
}

/// A "tab" is essentially an [`oro_mem::alloc::sync::Arc`] that can be indexed
/// by the global table and given a unique ID that can be shared, safely,
/// to userspace programs.
///
/// Once all references to a tab are dropped, the tab is removed from the
/// global table and the slot is reused with a new version number.
///
/// Further, tabs are read/write locked; they can be read from multiple
/// threads at once, but only one thread can write to a tab at a time.
///
/// This allows for safe and performant traversal of 'linked' tab items
/// (such as traversing up a thread -> instance -> ring, etc.).
pub struct Tab<T: Tabbed> {
	/// The Tab's ID.
	id:  u64,
	/// The raw pointer to the slot.
	ptr: *const Slot,
	/// Holds type `T`.
	_ty: PhantomData<T>,
}

// SAFETY: We can guarantee that this type is Send + Sync.
unsafe impl<T: Tabbed> Send for Tab<T> {}
// SAFETY: We can guarantee that this type is Send + Sync.
unsafe impl<T: Tabbed> Sync for Tab<T> {}

impl<T: Tabbed> Tab<T> {
	/// Creates the initial `Tab`, setting its users to 1.
	///
	/// # Safety
	/// If `MUST_BE_FRESH` is `false`, the caller MUST hold a VALID [`Tab`] or
	/// [`AnyTab`] reference to the same slot throughout the lifetime of this
	/// function call, until return. The user count is NOT CXC-protected unlike
	/// in [`AnyTab`].
	#[inline]
	unsafe fn new<const MUST_BE_FRESH: bool>(id: u64, ptr: *const Slot) -> Self {
		// SAFETY: We can guarantee that the slot is valid and the data is valid.
		let slot = unsafe { &*ptr };
		#[cfg(debug_assertions)]
		{
			debug_assert_eq!(slot.ty(), T::TY);

			if MUST_BE_FRESH {
				debug_assert_eq!(
					slot.lock.load(Acquire),
					0,
					"precondition failed: slot is locked (MUST_BE_FRESH=true)"
				);
				slot.users
					.compare_exchange(0, 1, Relaxed, Relaxed)
					.expect("precondition failed: slot users is not 0 (MUST_BE_FRESH=true)");
			} else {
				// SAFETY(qix-): Not CXC-protected; the caller has been instructed only
				// SAFETY(qix-): to call this function with `MUST_BE_FRESH=false` if they
				// SAFETY(qix-): have a valid reference to the slot already (i.e. an `AnyTab`).
				debug_assert_ne!(
					slot.users.fetch_add(1, Relaxed),
					0,
					"precondition failed: slot users is 0 (MUST_BE_FRESH=false)"
				);
			}
		}
		#[cfg(not(debug_assertions))]
		{
			if MUST_BE_FRESH {
				slot.users.store(1, Relaxed);
			} else {
				slot.users.fetch_add(1, Relaxed);
			}
		}

		Self {
			id,
			ptr,
			_ty: PhantomData,
		}
	}

	/// The tab's ID.
	#[must_use]
	#[inline(always)]
	pub fn id(&self) -> u64 {
		self.id
	}

	/// Allows a read-only view of the tab's data.
	#[inline]
	pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
		// SAFETY: We can guarantee that the slot is valid and the data is valid.
		let slot = unsafe { &*self.ptr };
		let guard = slot.read();
		// SAFETY: We control the allocation of the pointer, so this is safe.
		let data = unsafe { &*slot.data::<T>() };
		let r = f(data);
		// SAFETY(qix-): Keep the space ship running.
		drop(guard);
		r
	}

	/// Allows a mutable view of the tab's data.
	#[inline]
	pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
		// SAFETY: We can guarantee that the slot is valid and the data is valid.
		let slot = unsafe { &*self.ptr };
		let guard = slot.write();
		// SAFETY: We control the allocation of the pointer, so this is safe.
		let data = unsafe { &mut *slot.data::<T>() };
		let r = f(data);
		// SAFETY(qix-): Keep the space ship running.
		drop(guard);
		r
	}
}

impl<T: Tabbed> Clone for Tab<T> {
	#[inline(always)]
	fn clone(&self) -> Self {
		// SAFETY: We can guarantee that the slot is valid and the data is valid.
		let slot = unsafe { &*self.ptr };
		slot.users.fetch_add(1, Relaxed);
		Self {
			id:  self.id,
			ptr: self.ptr,
			_ty: PhantomData,
		}
	}
}

impl<T: Tabbed> Drop for Tab<T> {
	fn drop(&mut self) {
		// SAFETY: We can guarantee that the slot is valid and the data is valid.
		let slot = unsafe { &*self.ptr };
		if slot.users.fetch_sub(1, Relaxed) == 1 {
			// SAFETY: We have the only owning reference to this slot,
			// SAFETY: and we control its construction; it's always going to
			// SAFETY: be the slot pointed to by the tab's ID.
			unsafe {
				get().free(self.id, slot);
			}
			// SAFETY(qix-): THE SLOT IS NO LONGER OURS TO USE.
			// SAFETY(qix-): FURTHER ACCESS TO THE SLOT IS UNDEFINED BEHAVIOR.
		}
	}
}

/// A versioned slot within which to store a [`Tab`]'s data.
#[derive(Default)]
#[repr(C)]
struct Slot {
	/// The data stored in the slot. If the slot is free, it holds the next free slot
	/// in the list (the version bits should be treated as garbage and discarded upon
	/// claiming it).
	// **This must be the first field**.
	data: AtomicPtr<()>,
	/// The version and type of the slot.
	ver_ty: AtomicU64,
	/// The number of open [`Tab`]s.
	users:  AtomicU64,
	/// This reentrant lock has a few meanings:
	/// - If the high bit is clear, the lock is either free or has one or more readers (and no writers).
	/// - If the high bit is set, the lock is held by one (or more) writers **on the same core** (and no readers).
	lock:   AtomicU64,
}

const _: () = {
	oro_macro::assert_offset_of!(Slot, data, 0);
};

/// A list of slots, fit snugly into a page.
struct SlotList {
	/// The slots.
	slots: [Slot; 128],
}

impl Default for SlotList {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}

impl SlotList {
	/// Creates a new slot list.
	fn new() -> Self {
		assert::size_of::<Self, 4096>();
		let mut slots: [MaybeUninit<Slot>; 128] = MaybeUninit::uninit_array();
		for slot in &mut slots {
			slot.write(Slot::default());
		}
		Self {
			slots: unsafe { MaybeUninit::array_assume_init(slots) },
		}
	}
}

#[doc(hidden)]
const _: () = {
	assert::size_of::<SlotList, 4096>();
};

/// A scope guard for a reader lock on a slot.
struct SlotReaderGuard<'a> {
	/// The locked slot.
	slot: &'a Slot,
}

impl Drop for SlotReaderGuard<'_> {
	fn drop(&mut self) {
		#[cfg(debug_assertions)]
		{
			let loaded = self.slot.lock.load(Acquire);
			let kernel_id = (loaded >> 31) as u32;
			debug_assert!(
				loaded & (1 << 63) == 0,
				"precondition failed: slot is not locked for reading"
			);
			// SAFETY: This is just for debugging.
			let our_id = unsafe { crate::sync::oro_sync_current_core_id() };
			debug_assert!(
				kernel_id == our_id,
				"precondition failed: slot is not locked by this core: we are {our_id}, locked on \
				 {kernel_id}"
			);
			::oro_dbgutil::__oro_dbgutil_lock_release_reader(
				::core::ptr::from_ref(self.slot).addr(),
			);
		}
		let prev_value = self.slot.lock.fetch_sub(1, Release);
		#[cfg(debug_assertions)]
		{
			if prev_value & ((1 << 31) - 1) == 1 {
				self.slot
					.lock
					.compare_exchange(prev_value - 1, 0, Release, Relaxed)
					.expect("precondition failed: slot was modified during reader unlock");
			}
		}
		#[cfg(not(debug_assertions))]
		let _ = prev_value;
	}
}

/// A scope guard for a writer lock on a slot.
struct SlotWriterGuard<'a> {
	/// The locked slot.
	slot: &'a Slot,
}

impl Drop for SlotWriterGuard<'_> {
	fn drop(&mut self) {
		#[cfg(debug_assertions)]
		{
			let loaded = self.slot.lock.load(Acquire);
			let kernel_id = (loaded >> 31) as u32;
			debug_assert!(
				loaded & (1 << 63) != 0,
				"precondition failed: slot is not locked for writing"
			);
			// SAFETY: This is just for debugging.
			let our_id = unsafe { crate::sync::oro_sync_current_core_id() };
			debug_assert!(
				kernel_id == our_id,
				"precondition failed: slot is not locked by this core: we are {our_id}, locked on \
				 {kernel_id}"
			);
		}

		::oro_dbgutil::__oro_dbgutil_lock_release_writer(::core::ptr::from_ref(self.slot).addr());

		let prev_value = self.slot.lock.fetch_sub(1, Release);
		if prev_value & ((1 << 31) - 1) == 1 {
			// We were the last writer.
			#[cfg(not(debug_assertions))]
			{
				self.slot.lock.store(0, Release);
			}
			#[cfg(debug_assertions)]
			{
				self.slot
					.lock
					.compare_exchange(prev_value - 1, 0, Release, Relaxed)
					.expect("precondition failed: slot was modified during writer unlock");
			}
		}
	}
}

impl Slot {
	/// Returns the type of the slot.
	#[cfg_attr(debug_assertions, inline(always))]
	fn ty(&self) -> TabType {
		// SAFETY: We control all of the punning in this module, so
		// SAFETY: barring very blatant and bizarre misuse of the global table,
		// SAFETY: this should be safe.
		unsafe { ::core::mem::transmute::<u8, TabType>((self.ver_ty.load(Relaxed) >> 56) as u8) }
	}

	/// Marks the slot as free. Does not modify the version.
	///
	/// Returns `true` if the slot is now a tomb.
	///
	/// # Zombie Tombs
	/// By default, if the version overflows, the slot is marked as a "tomp"
	/// and the version is reset to 0. This is to prevent ABA problems in the long term.
	///
	/// However, most users will not experience any _real world_ side effects of allowing
	/// slot reuse, and may even benefit from it. It is up to whomever is building the
	/// kernel to decide this.
	///
	/// If slot reuse is to be allowed, the `zombie-tombs` feature must be enabled.
	/// This will wrap the version around to 0, allowing the slot to be reused at
	/// the small risk of ABA problems.
	///
	/// By default, Oro is "safe and secure by default", especially in the kernel.
	/// Therefore, the `zombie-tombs` feature is disabled by default. This means that
	/// building the kernel requires explicit opt-in to enable this behavior.
	///
	/// # Safety
	/// The slot MUST be free (from the caller's perspective).
	unsafe fn free_and_check_tomb(&self) -> bool {
		let old_ver_ty = self.ver_ty.load(Relaxed);
		let ver = old_ver_ty & MAX_VERSION_BEFORE_TOMBSTONE;
		let new_ver_ty = (ver & ((1 << 56) - 1)) | ((TabType::Free as u64) << 56);

		#[cfg(not(debug_assertions))]
		{
			self.ver_ty.store(new_ver_ty, Relaxed);
		}
		#[cfg(debug_assertions)]
		{
			self.ver_ty
				.compare_exchange(old_ver_ty, new_ver_ty, Relaxed, Relaxed)
				.expect("precondition failed: slot was modified during free");
		}

		#[expect(clippy::needless_bool)]
		if ver == MAX_VERSION_BEFORE_TOMBSTONE {
			#[cfg(feature = "zombie-tombs")]
			{
				false
			}
			#[cfg(not(feature = "zombie-tombs"))]
			{
				true
			}
		} else {
			false
		}
	}

	/// Unsafely returns the data stored in the slot
	/// as a pointer to `T`.
	///
	/// # Safety
	/// The slot MUST be occupied by a `T` type.
	#[inline(always)]
	unsafe fn data<T: Tabbed>(&self) -> *mut T {
		debug_assert!(self.ty() == T::TY);
		self.data.load(Acquire).cast()
	}

	/// Gets the next free slot.
	///
	/// # Safety
	/// The slot MUST be free. Callers are allowed to call this
	/// on a potentially occupied slot, as long as they are only
	/// doing so followed by a check-and-set of some head free list.
	///
	/// If this slot has never been allocated, this returns `0`.
	/// _DO NOT CALL THIS METHOD ON SLOTS THAT HAVE NEVER BEEN ALLOCATED._**
	/// `0` is a **valid slot ID** and is **not** a sentinel value.
	#[inline(always)]
	unsafe fn next_free(&self) -> u64 {
		self.data.load(Relaxed).addr() as u64
	}

	/// Sets the next free slot.
	///
	/// # Safety
	/// The slot MUST be free, and the passed `next` slot
	/// MUST be a valid FREE slot.
	#[inline(always)]
	unsafe fn set_next_free(&self, next: usize) {
		self.data.store(next as *mut (), Relaxed);
	}

	/// Unsafely sets the data stored in the slot and updates
	/// its type and version.
	///
	/// Returns the new version.
	///
	/// # Safety
	/// The slot MUST be free and NOT a tomb.
	///
	/// # Panics
	/// In debug mode, if the slot is not free (its type is not [`TabType::Free`]),
	/// this function will panic as a precondition failure.
	unsafe fn claim_unchecked<T: Tabbed>(&self, data: *mut T) -> u64 {
		let old_ver_ty = self.ver_ty.load(Relaxed);

		#[cfg(feature = "zombie-tombs")]
		let ver = (old_ver_ty + 1) & ((1 << 29) - 1);

		#[cfg(not(feature = "zombie-tombs"))]
		let ver = {
			let ver = (old_ver_ty + 1) & ((1 << 29) - 1);
			debug_assert_ne!(
				ver, 0,
				"tab version overflow (and kernel not built with zombie tombs)"
			);
			ver
		};

		let new_val = ver | ((T::TY as u64) << 56);
		#[cfg(debug_assertions)]
		{
			self.ver_ty
				.compare_exchange(old_ver_ty, new_val, Relaxed, Relaxed)
				.expect("precondition failed: slot was not free");
		}
		#[cfg(not(debug_assertions))]
		{
			self.ver_ty.store(new_val, Relaxed);
		}
		self.data.swap(data.cast(), Relaxed);
		ver
	}

	/// Attempts to return a reader guard for the slot.
	#[inline]
	fn try_read(&self) -> Option<SlotReaderGuard> {
		let loaded = self.lock.load(Acquire);

		let is_reader = loaded & (1 << 63) == 0;

		#[cfg(not(debug_assertions))]
		let (new_loaded, kernel_check) = (loaded, false);

		#[cfg(debug_assertions)]
		let (new_loaded, kernel_check) = {
			let kernel_id = (loaded >> 31) as u32;
			// SAFETY: There's nothing unsafe about this, it's just an extern prototype.
			let our_kernel_id = unsafe { crate::sync::oro_sync_current_core_id() };

			// SAFETY: There's nothing unsafe about this, it's just an extern prototype.
			if loaded > 0
				&& !is_reader
				&& kernel_id == unsafe { crate::sync::oro_sync_current_core_id() }
			{
				panic!(
					"precondition failed: slot is locked for writing by this core (do not call \
					 `with()` during a `with_mut()` on the same `Tab` handle): core {kernel_id}"
				);
			}

			(
				loaded | (u64::from(our_kernel_id) << 31),
				loaded != 0 && kernel_id != our_kernel_id,
			)
		};

		if !is_reader
			|| kernel_check
			|| self
				.lock
				.compare_exchange_weak(loaded, new_loaded + 1, Release, Relaxed)
				.is_err()
		{
			None
		} else {
			::oro_dbgutil::__oro_dbgutil_lock_acquire_reader(::core::ptr::from_ref(self).addr());
			Some(SlotReaderGuard { slot: self })
		}
	}

	/// Returns a reader guard for the slot, blocking until
	/// one is available.
	#[inline]
	fn read(&self) -> SlotReaderGuard {
		loop {
			if let Some(guard) = self.try_read() {
				return guard;
			}
		}
	}

	/// Attempts to return a writer guard for the slot.
	#[inline]
	fn try_write(&self) -> Option<SlotWriterGuard> {
		let loaded = self.lock.load(Acquire);
		let is_reader = loaded & (1 << 63) == 0;
		// 31 is intentional; we have 1 high bit to indicate writer status,
		// and 32 bits to store the kernel ID, which leaves the lower 31 bits
		// for the number of writers.
		let kernel_id = (loaded >> 31) as u32;

		#[cfg(debug_assertions)]
		{
			// SAFETY: There's nothing unsafe about this, it's just an extern prototype.
			if loaded > 0
				&& is_reader && kernel_id == unsafe { crate::sync::oro_sync_current_core_id() }
			{
				panic!(
					"precondition failed: slot is locked for reading by this core (do not call \
					 `with_mut()` during a `with()` on the same `Tab` handle): core {kernel_id}"
				);
			}
		}

		if (loaded > 0 && is_reader)
			// SAFETY: There's nothing unsafe about this, it's just an extern prototype.
			|| (!is_reader && kernel_id != unsafe { crate::sync::oro_sync_current_core_id() })
			|| (loaded & ((1 << 31) - 1)) == ((1 << 31) - 1)
			|| self
				.lock
				.compare_exchange_weak(
					loaded,
					(1 << 63) | (loaded + 1) | (u64::from(kernel_id) << 31),
					Release, Relaxed
				)
				.is_err()
		{
			None
		} else {
			::oro_dbgutil::__oro_dbgutil_lock_acquire_writer(::core::ptr::from_ref(self).addr());
			Some(SlotWriterGuard { slot: self })
		}
	}

	/// Returns a writer guard for the slot, blocking until
	/// one is available.
	#[inline]
	fn write(&self) -> SlotWriterGuard {
		loop {
			if let Some(guard) = self.try_write() {
				return guard;
			}
		}
	}
}

/// The type of value held in the tab slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TabType {
	/// The tab slot is free.
	Free = 0,
	/// A [`crate::ring::Ring`].
	Ring,
	/// An [`crate::instance::Instance`].
	Instance,
	/// A [`crate::thread::Thread`].
	Thread,
	/// A [`crate::interface::RingInterface`].
	RingInterface,
	/// A [`crate::module::Module`].
	Module,
}

impl<A: Arch> Tabbed for crate::thread::Thread<A> {
	const TY: TabType = TabType::Thread;
}

impl<A: Arch> Tabbed for crate::instance::Instance<A> {
	const TY: TabType = TabType::Instance;
}

impl<A: Arch> Tabbed for crate::module::Module<A> {
	const TY: TabType = TabType::Module;
}

impl<A: Arch> Tabbed for crate::ring::Ring<A> {
	const TY: TabType = TabType::Ring;
}

impl<A: Arch> Tabbed for crate::interface::RingInterface<A> {
	const TY: TabType = TabType::RingInterface;
}
