use crate::sync::{map_sync_mut, FairMutex};
use ::lazy_static::lazy_static;
use alloc::{collections::BTreeMap as Map, sync::Arc};

fn unique_id() -> usize {
	use ::core::sync::atomic::{AtomicUsize, Ordering};

	static COUNTER: AtomicUsize = AtomicUsize::new(0);

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

pub struct Ring {
	id: usize,
	parent: Option<Arc<Self>>,
}

lazy_static! {
	static ref RING_MAP: FairMutex<Map<usize, Arc<Ring>>> =
		FairMutex::new(Map::<usize, Arc<Ring>>::new());
}

impl Ring {
	fn new_with_parent(parent: Option<Arc<Self>>) -> Arc<Self> {
		let id = unique_id();

		let res = Arc::new(Self {
			id: id,
			parent: parent,
		});

		map_sync_mut(&*RING_MAP, |map| {
			debug_assert!(!map.contains_key(&id));
			map.insert(id, res.clone());
		});

		res
	}

	/// @note Should only be called by `oro::init()`
	pub fn new_root() -> Arc<Self> {
		Self::new_with_parent(None)
	}

	pub fn new(parent: Arc<Self>) -> Arc<Self> {
		Self::new_with_parent(Some(parent))
	}

	pub fn id(&self) -> usize {
		self.id
	}
}

pub fn get_ring_by_id(id: usize) -> Option<Arc<Ring>> {
	map_sync_mut(&*RING_MAP, |map| match map.get(&id) {
		None => None,
		Some(r) => Some(r.clone()),
	})
}
