//! Rings are collections of Modules, each with a single parent -
//! except for the "root ring" (ID=0), which has no parent (`None`).
//!
//! All module instances within a ring can see all other module
//! instances without restriction. They can also see all child
//! rings and their respective instances, also without restriction.
//!
//! Conversely, a module instance cannot traverse 'upward' and
//! look into any of the rings that parent its own. This forms
//! a security boundary by which isolation, access control,
//! organization, compartmentalization, "containerization",
//! and other useful scenarios may take place.
//!
//! Every ring is given a unique ID. In the nearly impossible
//! event wherein ring IDs are exhausted, the kernel will panic.

use crate::{sync, sync::UnfairRwMutex};
use ::core::mem::MaybeUninit;
#[cfg(debug_assertions)]
use ::core::sync::atomic::AtomicBool;
use ::core::sync::atomic::{AtomicUsize, Ordering};
use ::lazy_static::lazy_static;
use alloc::{
	collections::BTreeMap as Map,
	sync::{Arc, Weak},
};

#[doc(hidden)]
#[cfg(debug_assertions)]
static ROOT_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[doc(hidden)]
static mut ROOT_RING_DATA: MaybeUninit<Ring> = MaybeUninit::<Ring>::uninit();
lazy_static! {
	/// The map of all ring IDs to their respective ring data objects.
	///
	/// Values are stored as weak pointers to the internal
	/// data instances that can be promoted to reference counted
	/// [`Ring`] instances.
	static ref RING_MAP: UnfairRwMutex<Map<usize, WeakRingData>> =
		UnfairRwMutex::new(Map::<usize, WeakRingData>::new());
}

/// Internal data members for [`Ring`], which are just
/// wrappers around a reference counted [`RingData`] object.
struct RingData {
	/// The globally (kernel-wide) unique [`Ring`] ID number.
	///
	/// A [`Ring`] with the `id` of `0` refers to the "root ring",
	/// which is the first ring created by the kernel upon boot
	/// and has no `Self::parent`.
	id: usize,
	/// The parent [`Ring`], if any. This field is [`Some`] in all
	/// cases _expect_ the "root ring" (with a [`Self::id`] of `0`).
	///
	/// FIXME: This may not even be necessary to track. We don't currently
	/// use it, and the security model dictates that traversal upwards
	/// should never happen. I struggle to think of a case where it's
	/// useful to track this.
	#[deprecated(note = ".parent is probably not useful to track; don't rely on it being around")]
	parent: Option<WeakRingData>,
	/// A map of all child ring IDs to _owning_ child [`Ring`]s.
	children: Map<usize, Ring>,
}

/// Non-owning (weak) references to internal [`RingData`] objects.
type WeakRingData = Weak<UnfairRwMutex<RingData>>;
/// Owning (strong, counted) references to internal [`RingData`] objects.
type StrongRingData = Arc<UnfairRwMutex<RingData>>;

/// A wrapper for an owning [`StrongRingData`], which itself is just an
/// [`alloc::sync::Arc`] (atomically reference counted) [`RingData`]
/// object.
#[derive(Clone)]
pub struct Ring {
	#[doc(hidden)]
	data: StrongRingData,
}

impl Drop for RingData {
	fn drop(&mut self) {
		sync::map_write(&*RING_MAP, |map| {
			map.remove(&self.id);
		});
	}
}

impl Ring {
	/// Creates a new instance with the given ID and parent.
	///
	/// # Arguments
	///
	/// * `id` - A globally (whole-kernel) unique ID. Can only be
	///   `0` in the case of the "root ring".
	///
	/// * `parent` - A parent [`Ring`] in the case that `id != 0`
	fn new_with_parent(id: usize, parent: Option<Self>) -> Self {
		debug_assert!(id == 0 || parent.is_some());

		let res = Self {
			data: Arc::new(UnfairRwMutex::new(RingData {
				id,
				parent: parent.map(|parent| Arc::downgrade(&parent.data)),
				children: Map::new(),
			})),
		};

		sync::map_write(&*RING_MAP, |map| {
			debug_assert!(!map.contains_key(&id));
			map.insert(id, Arc::downgrade(&res.data));
		});

		res
	}

	/// Creates a new child [`Ring`] instance
	pub fn new(parent: Self) -> Self {
		let id = unique_id();
		debug_assert!(id != 0);

		let parent_data = parent.data.clone();
		let res = Self::new_with_parent(id, Some(parent));

		sync::map_write(&parent_data, |parent| {
			parent.children.insert(id, res.clone())
		});

		res
	}

	/// The [`Ring`]'s global (kernel-wide) unique ID.
	/// An ID of `0` indicates this is the "root ring".
	pub fn id(&self) -> usize {
		sync::map_read(&self.data, |this| this.id)
	}
}

/// Initializes the "root ring".
///
/// # Unsafe
///
/// Must ONLY be called ONCE, and MUST ALWAYS be called BEFORE [`drop_root()`].
///
/// **This function is intended _only_ to be called by [`crate::oro::init`].
/// DO NOT CALL THIS FUNCTION DIRECTLY.**
///
/// Debug builds enforce this constraint.
pub unsafe fn init_root() {
	#[cfg(debug_assertions)]
	{
		if ROOT_INITIALIZED
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_err()
		{
			panic!("oro::root::init_root() called but root ring already initialized!");
		}
	}

	ROOT_RING_DATA.write(Ring::new_with_parent(0, None));
}

/// Drops (disposes) the "root ring". **This effectively disposes the entire
/// system, including all rings, modules, resources, and userspace memory.**
///
/// # Unsafe
///
/// Must ONLY be called ONCE, and MUST ALWAYS be called AFTER a successful
/// call to [`init_root()`].
///
/// **This function is intended _only_ to be called by [`crate::oro::init`].
/// DO NOT CALL THIS FUNCTION DIRECTLY.**
///
/// Debug builds enforce this constraint.
pub unsafe fn drop_root() {
	#[cfg(debug_assertions)]
	{
		if ROOT_INITIALIZED
			.compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
			.is_err()
		{
			panic!("oro::root::drop_root() called but root ring already initialized!");
		}
	}

	ROOT_RING_DATA.assume_init_drop();
}

/// Returns the "root ring"
///
/// # Unsafe
///
/// Must ONLY be called _after_ a call to [`init_root`] and _before_
/// a matching call to [`drop_root`].
///
/// For the most part, if you're working outside the init functions (either
/// Oro kernel or architecture-specific init functions), this constraint
/// will be satisfied.
///
/// Debug builds enforce this constraint.
pub fn root() -> Ring {
	#[cfg(debug_assertions)]
	if !ROOT_INITIALIZED.load(Ordering::Acquire) {
		panic!("oro::root::root() called but root ring isn't initialized!");
	}

	(unsafe { ROOT_RING_DATA.assume_init_ref() }).clone()
}

/// Get a ring by its ID.
///
/// # Arguments
///
/// * `id` - The ring ID (if you know you're passing `0`, use [`root`]() instead)
#[allow(unused)]
pub fn get_ring_by_id(id: usize) -> Option<Ring> {
	sync::map_read(&*RING_MAP, |map| {
		map.get(&id)
			.and_then(|weak_data| weak_data.upgrade())
			.map(|strong_data| Ring { data: strong_data })
	})
}

/// Returns a guaranteed-unique ID for new [`Ring`]s.
fn unique_id() -> usize {
	// NOTE: Imperative that this starts at 1!
	#[doc(hidden)]
	static COUNTER: AtomicUsize = AtomicUsize::new(1);

	let new_id = COUNTER.fetch_add(1, Ordering::Relaxed);

	if new_id == usize::MAX {
		// On a 32-bit machine, you would need to
		// allocate 27 per second for 5 years
		// in order to overflow.
		//
		// On a 64-bit machine, you would need to
		// allocate 584.9 million per second for
		// 1000 years in order to overflow.
		//
		// Still want to be complete, though.
		panic!("ring ID allocator overflowed");
	} else {
		new_id
	}
}
