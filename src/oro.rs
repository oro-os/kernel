mod module;
mod ring;

use ::alloc::sync::Arc;
use ring::Ring;

static mut ROOT_RING: Option<Arc<Ring>> = None;

pub fn init() {
	println!(
		"booting Oro {}-{}",
		env!("CARGO_PKG_VERSION"),
		if cfg!(debug_assertions) { "d" } else { "r" }
	);

	// Allocate the root ring
	unsafe {
		debug_assert!(ROOT_RING.is_none());
		ROOT_RING = Some(Ring::new_root());
		debug_assert!(ROOT_RING.as_mut().unwrap().id() == 0usize);
		debug_assert!(ROOT_RING.as_mut().unwrap().id() == ring::get_ring_by_id(0).unwrap().id());
	}
	println!("root ring initialized");
}
