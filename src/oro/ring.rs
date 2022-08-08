use crate::{sync, sync::UnfairRwMutex};
use ::lazy_static::lazy_static;
use alloc::{
	collections::BTreeMap as Map,
	sync::{Arc, Weak},
};

fn unique_id() -> usize {
	use ::core::sync::atomic::{AtomicUsize, Ordering};

	// NOTE: Imperative that this starts at 1!
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

struct RingData {
	id: usize,
	parent: Option<WeakRingData>,
	children: Map<usize, Ring>,
}

type WeakRingData = Weak<UnfairRwMutex<RingData>>;
type StrongRingData = Arc<UnfairRwMutex<RingData>>;

#[derive(Clone)]
pub struct Ring {
	data: StrongRingData,
}

lazy_static! {
	static ref RING_MAP: UnfairRwMutex<Map<usize, WeakRingData>> =
		UnfairRwMutex::new(Map::<usize, WeakRingData>::new());
}

impl Ring {
	fn new_with_parent(id: usize, parent: Option<Self>) -> Self {
		let res = Self {
			data: Arc::new(UnfairRwMutex::new(RingData {
				id: id,
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

	pub fn root() -> Self {
		Self::new_with_parent(0, None)
	}

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

	pub fn id(&self) -> usize {
		sync::map_read(&self.data, |this| this.id)
	}
}

impl Drop for RingData {
	fn drop(&mut self) {
		sync::map_write(&*RING_MAP, |map| {
			map.remove(&self.id);
		});
	}
}

#[allow(unused)]
pub fn get_ring_by_id(id: usize) -> Option<Ring> {
	sync::map_read(&*RING_MAP, |map| {
		map.get(&id)
			.and_then(|weak_data| weak_data.upgrade())
			.map(|strong_data| Ring { data: strong_data })
	})
}
